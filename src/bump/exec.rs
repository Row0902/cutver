use crate::atomic;
use crate::bump::{Change, Drift, Error, Summary, Touched};
use crate::changelog;
use crate::cli::BumpLevel;
use crate::config::Config;
use crate::conventional;
use crate::git;
use crate::manifest;
use crate::preflight;
use crate::semver_bump::{self, Bump};
use semver::Version;
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::Path;
use std::process::Child;
use std::thread;
use std::time::{Duration, Instant};

pub fn run(
    config: &Config,
    bump_kind: impl Into<BumpLevel>,
    dry_run: bool,
    skip_preflight: &[String],
) -> Result<Summary, Error> {
    // cutver runs from the project root where release.toml lives; the git
    // repository root is therefore the directory containing release.toml.
    let repo = &config.root_dir;
    git::require_clean_tree(repo, config.git.require_clean_tree)?;
    git::require_branch(repo, config.git.require_branch.as_deref())?;

    let (source_entry, _editor, current) = current_source(config)?;
    let bump_level = bump_kind.into();
    let mut auto_commits = None;
    let bump_semver = match bump_level {
        BumpLevel::Patch => Bump::Patch,
        BumpLevel::Minor => Bump::Minor,
        BumpLevel::Major => Bump::Major,
        BumpLevel::Auto => {
            let tag = git::latest_tag(repo, Some(&config.git.tag_prefix))?;
            let commits = git::commits_since(repo, tag.as_deref())?;
            let (deduced, parsed) = conventional::parse_and_deduce_bump(&commits);
            auto_commits = Some(parsed);
            deduced
        }
    };
    let next = semver_bump::bump(&current, bump_semver);
    let commit_message = git::commit_message(&config.git.commit_message, &next.to_string());
    let tag = git::tag_name(&config.git.tag_prefix, &next.to_string());

    // Abort before any mutation whenever the target tag already exists. HEAD
    // moves during the run, so "points at HEAD" is not stable here: a tag at
    // HEAD today sits behind the release commit created minutes later. Fail
    // closed with an actionable message; the maintainer resolves the tag
    // (delete it or bump deliberately) before re-running.
    if git::tag_exists(repo, &tag)? {
        let at = git::rev_parse(repo, &format!("{tag}^{{commit}}"))?;
        return Err(Error::TagExists {
            tag: tag.clone(),
            commit: at,
        });
    }

    let preflight_plan = preflight::plan(&config.preflight, skip_preflight);
    if !dry_run {
        preflight::run(&preflight_plan)?;
    }

    // Phase 1: compute every new manifest content in memory. No disk writes.
    let computed = compute(config, &next)?;

    // Phase 2: write all changed manifests atomically. On failure, best-effort
    // rollback restores previously written files to their original content.
    let (touched, mut paths_to_stage) = apply(&computed, &next, dry_run)?;

    let original_changelog = if let Some(cl_path) = &config.changelog.path {
        let original = if !dry_run { read(cl_path).ok() } else { None };
        if !dry_run {
            let update_res = if config.changelog.mode == "template" {
                changelog::update(cl_path, &tag, &config.changelog.entry_template)
            } else {
                let commits = match auto_commits {
                    Some(parsed) => parsed,
                    None => {
                        let latest_tag = match git::latest_tag(repo, Some(&config.git.tag_prefix)) {
                            Ok(t) => t,
                            Err(e) => {
                                rollback(&computed, &paths_to_stage, None);
                                return Err(Error::Git(e));
                            }
                        };
                        let commit_msgs = match git::commits_since(repo, latest_tag.as_deref()) {
                            Ok(c) => c,
                            Err(e) => {
                                rollback(&computed, &paths_to_stage, None);
                                return Err(Error::Git(e));
                            }
                        };
                        let (_bump, parsed) = conventional::parse_and_deduce_bump(&commit_msgs);
                        parsed
                    }
                };
                let body = changelog::render_body(&config.changelog, &commits);
                changelog::update(cl_path, &tag, &body)
            };
            if let Err(e) = update_res {
                rollback(&computed, &paths_to_stage, None);
                return Err(Error::Changelog(e));
            }
        }
        paths_to_stage.push(cl_path.clone());
        original.map(|orig| (cl_path.clone(), orig))
    } else {
        None
    };

    let summary_post_bump = if let Some(raw_post_bump) = &config.hooks.post_bump {
        let cmd = format_command(raw_post_bump, &next.to_string(), &tag);
        if !dry_run {
            let (shell, flag) = if cfg!(windows) { ("cmd", "/C") } else { ("sh", "-c") };
            let status = std::process::Command::new(shell)
                .arg(flag)
                .arg(&cmd)
                .current_dir(&config.root_dir)
                .status()
                .map_err(|e| {
                    rollback(&computed, &paths_to_stage, original_changelog.as_ref());
                    Error::PostBumpHookSpawn {
                        command: cmd.clone(),
                        source: e,
                    }
                })?;
            if !status.success() {
                rollback(&computed, &paths_to_stage, original_changelog.as_ref());
                return Err(Error::PostBumpHookFailed { command: cmd, status });
            }
            let modified = match git::status_files(repo) {
                Ok(m) => m,
                Err(e) => {
                    rollback(&computed, &paths_to_stage, original_changelog.as_ref());
                    return Err(Error::Git(e));
                }
            };
            for f in modified {
                let rel_f = Path::new(&f);
                if !is_known_lockfile(rel_f) {
                    continue;
                }
                let abs_f = repo.join(rel_f);
                let already_staged = paths_to_stage.iter().any(|p| {
                    let p_path = Path::new(p);
                    p_path == rel_f || p_path == abs_f
                });
                if !already_staged {
                    paths_to_stage.push(f);
                }
            }
        }
        Some(cmd)
    } else {
        None
    };

    git::stage(repo, &paths_to_stage, dry_run).map_err(|e| {
        rollback(&computed, &paths_to_stage, original_changelog.as_ref());
        unstage(repo);
        Error::Stage(e)
    })?;
    git::commit(repo, &commit_message, dry_run).map_err(|e| {
        rollback(&computed, &paths_to_stage, original_changelog.as_ref());
        unstage(repo);
        Error::Commit(e)
    })?;
    let tag_report = git::tag(repo, &tag, &next.to_string(), dry_run).map_err(|e| Error::Tag {
        tag: tag.clone(),
        source: e,
    })?;
    let tag_skipped = tag_report.is_some() && !dry_run;

    let publish_push_command = if config.publish.push {
        git::push(repo, config.git.require_branch.as_deref(), true, dry_run).map_err(Error::Push)?
    } else {
        None
    };

    let mut publish_commands = Vec::new();
    for cmd in &config.publish.commands {
        let formatted = format_command(cmd, &next.to_string(), &tag);
        if !dry_run {
            run_publish_command(&formatted, &config.root_dir, config.publish.default_timeout)?;
        }
        publish_commands.push(formatted);
    }

    Ok(Summary {
        source: source_entry.path.clone(),
        current,
        next,
        dry_run,
        preflight: preflight_plan,
        touched,
        changelog: config.changelog.path.clone(),
        commit_message,
        tag,
        tag_skipped,
        post_bump: summary_post_bump,
        publish_push: config.publish.push,
        publish_push_command,
        publish_commands,
    })
}

fn format_command(template: &str, version: &str, tag: &str) -> String {
    template.replace("{version}", version).replace("{tag}", tag)
}

/// Single source of truth for mapping `config.version.current_source` to its
/// manifest entry, editor, and current version. Used by both `run` and `doctor`
/// to eliminate the previously duplicated lookup and error-mapping blocks.
fn current_source(
    config: &Config,
) -> Result<(&crate::config::Manifest, Box<dyn manifest::ManifestEditor>, Version), Error> {
    let entry = config
        .manifest
        .iter()
        .find(|m| m.path == config.version.current_source)
        .ok_or_else(|| Error::Read {
            path: config.version.current_source.clone(),
            source: io::Error::new(io::ErrorKind::NotFound, "current_source manifest entry not found"),
        })?;
    let content = read(&entry.path)?;
    let editor = manifest::editor_for(entry).map_err(|e| Error::CurrentSource {
        path: entry.path.clone(),
        source: e,
    })?;
    let version = editor.read_version(&content).map_err(|e| Error::CurrentSource {
        path: entry.path.clone(),
        source: e,
    })?;
    Ok((entry, editor, version))
}

fn read_manifest(m: &crate::config::Manifest) -> Result<(Box<dyn manifest::ManifestEditor>, String, Version), Error> {
    let content = read(&m.path)?;
    let editor = manifest::editor_for(m).map_err(|e| Error::Manifest {
        path: m.path.clone(),
        source: e,
    })?;
    let version = editor.read_version(&content).map_err(|e| Error::Manifest {
        path: m.path.clone(),
        source: e,
    })?;
    Ok((editor, content, version))
}

fn compute(config: &Config, next: &Version) -> Result<Vec<Change>, Error> {
    let mut out = Vec::new();
    for m in &config.manifest {
        let (editor, content, old) = read_manifest(m)?;
        let new = editor.write_version(&content, next).map_err(|e| Error::Manifest {
            path: m.path.clone(),
            source: e,
        })?;
        let changed = content != new;
        out.push(Change {
            path: m.path.clone(),
            old,
            original: content,
            new,
            changed,
        });
    }
    Ok(out)
}

fn apply(computed: &[Change], next: &Version, dry_run: bool) -> Result<(Vec<Touched>, Vec<String>), Error> {
    let mut touched = Vec::new();
    let mut paths = Vec::new();
    let mut written: Vec<usize> = Vec::new();
    for (i, c) in computed.iter().enumerate() {
        touched.push(Touched {
            path: c.path.clone(),
            old: c.old.to_string(),
            new: next.to_string(),
        });
        if !c.changed {
            continue;
        }
        if !dry_run {
            if let Err(e) = atomic::write_atomic(&c.path, &c.new) {
                let mut rollback = String::new();
                for &wi in &written {
                    if let Err(re) = atomic::write_atomic(&computed[wi].path, &computed[wi].original) {
                        rollback.push_str(&format!("failed to restore {}: {}; ", computed[wi].path, re));
                    }
                }
                if rollback.is_empty() {
                    rollback = "all restored.".into();
                }
                return Err(Error::WriteRollback {
                    path: c.path.clone(),
                    source: e,
                    rollback,
                });
            }
            written.push(i);
        }
        paths.push(c.path.clone());
    }
    Ok((touched, paths))
}

fn rollback(computed: &[Change], paths: &[String], changelog: Option<&(String, String)>) {
    let set: HashSet<&str> = paths.iter().map(|s| s.as_str()).collect();
    for c in computed {
        if set.contains(c.path.as_str()) {
            let _ = atomic::write_atomic(&c.path, &c.original);
        }
    }
    if let Some((path, orig)) = changelog
        && set.contains(path.as_str())
    {
        let _ = atomic::write_atomic(path, orig);
    }
}

fn unstage(repo: &Path) {
    let _ = std::process::Command::new("git")
        .current_dir(repo)
        .args(["reset", "HEAD", "--quiet"])
        .status();
}

pub fn doctor(config: &Config) -> Result<Vec<Drift>, Error> {
    let (source_entry, _editor, expected) = current_source(config)?;
    let mut drifts = Vec::new();
    for m in &config.manifest {
        if m.path == source_entry.path {
            continue;
        }
        let (_editor, _content, actual) = read_manifest(m)?;
        if actual != expected {
            drifts.push(Drift {
                path: m.path.clone(),
                expected: expected.to_string(),
                actual: actual.to_string(),
            });
        }
    }
    Ok(drifts)
}

fn read(path: impl AsRef<Path>) -> Result<String, Error> {
    let path = path.as_ref();
    fs::read_to_string(path).map_err(|e| Error::Read {
        path: path.display().to_string(),
        source: e,
    })
}

const KNOWN_LOCKFILES: &[&str] = &[
    "Cargo.lock",
    "package-lock.json",
    "pnpm-lock.yaml",
    "yarn.lock",
    "bun.lockb",
    "gradle.lockfile",
    "poetry.lock",
    "Pipfile.lock",
    "composer.lock",
];

fn is_known_lockfile(path: impl AsRef<Path>) -> bool {
    path.as_ref()
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|name| KNOWN_LOCKFILES.contains(&name))
}

fn run_publish_command(command: &str, current_dir: &Path, timeout_secs: Option<u64>) -> Result<(), Error> {
    let timeout = timeout_secs.map(Duration::from_secs);
    let start = Instant::now();
    let mut child = spawn_command(command, current_dir).map_err(|e| Error::PublishCommandSpawn {
        command: command.to_string(),
        source: e,
    })?;

    loop {
        match child.try_wait().map_err(|e| Error::PublishCommandSpawn {
            command: command.to_string(),
            source: e,
        })? {
            Some(status) => {
                if status.success() {
                    return Ok(());
                }
                return Err(Error::PublishCommandFailed {
                    command: command.to_string(),
                    status,
                });
            }
            None => {
                if let Some(limit) = timeout {
                    let elapsed = start.elapsed();
                    if elapsed >= limit {
                        kill_tree(&mut child);
                        return Err(Error::PublishCommandTimeout {
                            command: command.to_string(),
                            timeout: limit.as_secs(),
                            elapsed_ms: elapsed.as_millis(),
                        });
                    }
                    thread::sleep(POLL.min(limit - elapsed));
                } else {
                    thread::sleep(POLL);
                }
            }
        }
    }
}

const POLL: Duration = Duration::from_millis(50);

fn spawn_command(command: &str, current_dir: &Path) -> Result<Child, io::Error> {
    let (shell, flag) = shell();
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            std::process::Command::new(shell)
                .arg(flag)
                .arg(command)
                .current_dir(current_dir)
                .pre_exec(|| {
                    let _ = setpgid(0, 0);
                    Ok(())
                })
                .spawn()
        }
    }
    #[cfg(not(unix))]
    {
        std::process::Command::new(shell)
            .arg(flag)
            .arg(command)
            .current_dir(current_dir)
            .spawn()
    }
}

fn kill_tree(child: &mut Child) {
    #[cfg(unix)]
    unsafe {
        let _ = killpg(child.id() as i32, SIGKILL);
    }
    #[cfg(windows)]
    {
        let pid = child.id().to_string();
        let _ = std::process::Command::new("taskkill")
            .args(["/F", "/T", "/PID", pid.as_str()])
            .output();
        let _ = child.kill();
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = child.kill();
    }
    let _ = child.wait();
}

#[cfg(unix)]
const SIGKILL: i32 = 9;
#[cfg(unix)]
unsafe extern "C" {
    fn setpgid(pid: i32, pgid: i32) -> i32;
    fn killpg(pgrp: i32, sig: i32) -> i32;
}

#[cfg(unix)]
fn shell() -> (&'static str, &'static str) {
    ("sh", "-c")
}
#[cfg(windows)]
fn shell() -> (&'static str, &'static str) {
    ("cmd", "/C")
}
#[cfg(not(any(unix, windows)))]
fn shell() -> (&'static str, &'static str) {
    ("sh", "-c")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(unix)]
    fn test_run_publish_command_times_out() {
        let temp = std::env::temp_dir();
        let start = Instant::now();
        let res = run_publish_command("sleep 5", &temp, Some(1));
        assert!(start.elapsed() < Duration::from_secs(3));
        match res {
            Err(Error::PublishCommandTimeout {
                command,
                timeout,
                elapsed_ms,
            }) => {
                assert_eq!(command, "sleep 5");
                assert_eq!(timeout, 1);
                assert!(elapsed_ms >= 1000);
            }
            other => panic!("expected PublishCommandTimeout, got {:?}", other),
        }
    }

    #[test]
    fn test_known_lockfiles_matched() {
        for lockfile in KNOWN_LOCKFILES {
            assert!(is_known_lockfile(Path::new(lockfile)));
            assert!(is_known_lockfile(Path::new("subdir").join(lockfile)));
            assert!(is_known_lockfile(Path::new("deep/nested/path").join(lockfile)));
        }
    }

    #[test]
    fn test_arbitrary_files_not_matched() {
        let non_lockfiles = [
            "unrelated.txt",
            "Cargo.toml",
            "package.json",
            "release.toml",
            "cutver.toml",
            "Cargo.lock.backup",
            "not-Cargo.lock",
            "lockfile",
            "gradle.lock",
            "poetry.lock.bak",
            "",
        ];
        for f in non_lockfiles {
            assert!(!is_known_lockfile(Path::new(f)));
        }
    }
}
