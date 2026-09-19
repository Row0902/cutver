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
    let bump_semver = match bump_level {
        BumpLevel::Patch => Bump::Patch,
        BumpLevel::Minor => Bump::Minor,
        BumpLevel::Major => Bump::Major,
        BumpLevel::Auto => {
            let tag = git::latest_tag(repo, Some(&config.git.tag_prefix))?;
            let commits = git::commits_since(repo, tag.as_deref())?;
            let (deduced, _parsed) = conventional::parse_and_deduce_bump(&commits);
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
        if !dry_run && let Err(e) = changelog::update(cl_path, &tag, &config.changelog.entry_template) {
            rollback(&computed, &paths_to_stage, None);
            return Err(Error::Changelog(e));
        }
        paths_to_stage.push(cl_path.clone());
        original.map(|orig| (cl_path.clone(), orig))
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
    })
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
