use std::io;
use std::path::Path;
use std::process::{Command, ExitStatus, Output};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("guard failed: {detail}")]
    Guard { detail: String },
    #[error("git command '{command}' failed to run: {source}")]
    Command { command: String, source: io::Error },
    #[error("git command '{command}' exited with status {status}")]
    Status { command: String, status: ExitStatus },
    #[error("failed to read git output: {source}")]
    Output { source: io::Error },
}

fn run_git(repo: impl AsRef<Path>, args: &[&str]) -> Result<Output, Error> {
    Command::new("git")
        .current_dir(repo)
        .args(args)
        .output()
        .map_err(|e| Error::Command {
            command: args.join(" "),
            source: e,
        })
}

fn stdout_text(output: Output, command: &str) -> Result<String, Error> {
    if !output.status.success() {
        return Err(Error::Status {
            command: command.into(),
            status: output.status,
        });
    }
    String::from_utf8(output.stdout).map_err(|e| Error::Output {
        source: io::Error::new(io::ErrorKind::InvalidData, e),
    })
}

pub fn tag_exists(repo: impl AsRef<Path>, tag_name: &str) -> Result<bool, Error> {
    let output = run_git(&repo, &["tag", "-l", tag_name])?;
    if !output.status.success() {
        return Err(Error::Status {
            command: format!("tag -l {tag_name}"),
            status: output.status,
        });
    }
    Ok(!String::from_utf8_lossy(&output.stdout).trim().is_empty())
}

pub fn rev_parse(repo: impl AsRef<Path>, rev: &str) -> Result<String, Error> {
    let text = stdout_text(run_git(&repo, &["rev-parse", rev])?, &format!("rev-parse {rev}"))?;
    Ok(text.trim().to_string())
}

pub fn is_clean(porcelain: &str) -> bool {
    porcelain.trim().is_empty()
}

pub fn parse_branch(output: &str) -> &str {
    output.trim()
}

pub fn require_clean_tree(repo: impl AsRef<Path>, require: bool) -> Result<(), Error> {
    if !require {
        return Ok(());
    }
    let text = stdout_text(run_git(&repo, &["status", "--porcelain"])?, "status --porcelain")?;
    if is_clean(&text) {
        Ok(())
    } else {
        Err(Error::Guard {
            detail: format!("working tree has uncommitted changes:\n{text}"),
        })
    }
}

pub fn require_branch(repo: impl AsRef<Path>, expected: Option<&str>) -> Result<(), Error> {
    if let Some(branch) = expected {
        let current = stdout_text(
            run_git(&repo, &["symbolic-ref", "--short", "HEAD"])?,
            "symbolic-ref --short HEAD",
        )?;
        let current = parse_branch(&current);
        if current != branch {
            return Err(Error::Guard {
                detail: format!("on branch '{current}', expected '{branch}'"),
            });
        }
    }
    Ok(())
}

pub fn stage(repo: impl AsRef<Path>, paths: &[String], dry_run: bool) -> Result<Option<String>, Error> {
    if paths.is_empty() {
        return Ok(None);
    }
    let mut args = vec!["add".to_string()];
    args.extend(paths.iter().cloned());
    if dry_run {
        return Ok(Some(format!("git add {}", paths.join(" "))));
    }
    let status = Command::new("git")
        .current_dir(&repo)
        .args(&args)
        .status()
        .map_err(|e| Error::Command {
            command: args.join(" "),
            source: e,
        })?;
    if status.success() {
        Ok(None)
    } else {
        Err(Error::Status {
            command: args.join(" "),
            status,
        })
    }
}

pub fn commit(repo: impl AsRef<Path>, message: &str, dry_run: bool) -> Result<Option<String>, Error> {
    if dry_run {
        return Ok(Some(format!(r#"git commit -m "{}""#, message)));
    }
    let status = Command::new("git")
        .current_dir(&repo)
        .args(["commit", "-m", message])
        .status()
        .map_err(|e| Error::Command {
            command: "commit".into(),
            source: e,
        })?;
    if status.success() {
        Ok(None)
    } else {
        Err(Error::Status {
            command: "commit".into(),
            status,
        })
    }
}

pub fn tag(repo: impl AsRef<Path>, tag_name: &str, version: &str, dry_run: bool) -> Result<Option<String>, Error> {
    let msg = format!("Release {version}");
    if dry_run {
        return Ok(Some(format!(r#"git tag -a {tag_name} -m "{msg}""#)));
    }
    if tag_exists(&repo, tag_name)? {
        let tag_commit = rev_parse(&repo, &format!("{tag_name}^{{commit}}"))?;
        let head = rev_parse(&repo, "HEAD")?;
        if tag_commit == head {
            return Ok(Some(format!("skipped (already points at HEAD): {tag_name}")));
        }
        return Err(Error::Guard {
            detail: format!("tag '{tag_name}' already exists at {tag_commit}, not HEAD ({head})"),
        });
    }
    let status = Command::new("git")
        .current_dir(&repo)
        .args(["tag", "-a", tag_name, "-m", &msg])
        .status()
        .map_err(|e| Error::Command {
            command: "tag".into(),
            source: e,
        })?;
    if status.success() {
        Ok(None)
    } else {
        Err(Error::Status {
            command: "tag".into(),
            status,
        })
    }
}

pub fn push(
    repo: impl AsRef<Path>,
    branch: Option<&str>,
    include_tags: bool,
    dry_run: bool,
) -> Result<Option<String>, Error> {
    if dry_run {
        return Ok(Some(
            format!(
                "git push origin {} {}",
                branch.unwrap_or("HEAD"),
                if include_tags { "--tags" } else { "" }
            )
            .trim()
            .to_string(),
        ));
    }
    let target = branch.unwrap_or("HEAD");
    let mut args = vec!["push", "origin", target];
    if include_tags {
        args.push("--tags");
    }
    let status = Command::new("git")
        .current_dir(&repo)
        .args(&args)
        .status()
        .map_err(|e| Error::Command {
            command: args.join(" "),
            source: e,
        })?;
    if status.success() {
        Ok(None)
    } else {
        Err(Error::Status {
            command: args.join(" "),
            status,
        })
    }
}

pub fn status_files(repo: impl AsRef<Path>) -> Result<Vec<String>, Error> {
    let text = stdout_text(run_git(&repo, &["status", "--porcelain"])?, "status --porcelain")?;
    let mut files = Vec::new();
    for line in text.lines() {
        if line.len() >= 4 {
            let mut file_path = line[3..].trim();
            if file_path.starts_with('"') && file_path.ends_with('"') && file_path.len() >= 2 {
                file_path = &file_path[1..file_path.len() - 1];
            }
            if let Some((_, new_path)) = file_path.split_once(" -> ") {
                file_path = new_path.trim();
            }
            if !file_path.is_empty() {
                files.push(file_path.to_string());
            }
        }
    }
    Ok(files)
}

pub fn commit_message(template: &str, version: &str) -> String {
    template.replace("{version}", version)
}

pub fn tag_name(prefix: &str, version: &str) -> String {
    format!("{prefix}{version}")
}

pub fn latest_tag(repo: impl AsRef<Path>, tag_prefix: Option<&str>) -> Result<Option<String>, Error> {
    let mut args = vec!["describe", "--tags", "--abbrev=0"];
    let match_arg;
    if let Some(prefix) = tag_prefix
        && !prefix.is_empty()
    {
        match_arg = format!("{prefix}*");
        args.push("--match");
        args.push(&match_arg);
    }
    let output = run_git(&repo, &args)?;
    if !output.status.success() {
        return Ok(None);
    }
    let tag = String::from_utf8(output.stdout)
        .map_err(|e| Error::Output {
            source: io::Error::new(io::ErrorKind::InvalidData, e),
        })?
        .trim()
        .to_string();
    if tag.is_empty() { Ok(None) } else { Ok(Some(tag)) }
}

pub fn commits_since(repo: impl AsRef<Path>, tag: Option<&str>) -> Result<Vec<String>, Error> {
    let range;
    let mut args = vec!["log"];
    if let Some(t) = tag {
        range = format!("{t}..HEAD");
        args.push(&range);
    }
    args.push("--format=%B%x00");
    let output = run_git(&repo, &args)?;
    if !output.status.success() {
        if tag.is_none() {
            return Ok(Vec::new());
        }
        return Err(Error::Status {
            command: args.join(" "),
            status: output.status,
        });
    }
    let text = String::from_utf8(output.stdout).map_err(|e| Error::Output {
        source: io::Error::new(io::ErrorKind::InvalidData, e),
    })?;
    let commits = text
        .split('\0')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    Ok(commits)
}

pub fn init_test_repo(dir: impl AsRef<std::path::Path>) {
    let dir = dir.as_ref();
    let run = |args: &[&str]| {
        let status = std::process::Command::new("git")
            .current_dir(dir)
            .args(args)
            .status()
            .expect("failed to execute git");
        assert!(status.success(), "git command failed: {:?}", args);
    };
    run(&["init"]);
    run(&["config", "user.email", "test@example.com"]);
    run(&["config", "user.name", "Test User"]);
    run(&["config", "commit.gpgsign", "false"]);
    run(&["config", "tag.gpgsign", "false"]);
}

#[cfg(test)]
mod tests {
    static TMP_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    fn tmp_id(prefix: &str) -> String {
        let n = TMP_SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        format!("{}-{}-{}", prefix, std::process::id(), n)
    }

    use super::*;
    use std::fs;

    fn tmp_repo() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(tmp_id("cutver-git"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        for a in [
            &["init", "-q"] as &[&str],
            &["config", "user.email", "t@e.com"],
            &["config", "user.name", "T"],
            &["config", "commit.gpgsign", "false"],
            &["config", "tag.gpgsign", "false"],
        ] {
            assert!(
                Command::new("git")
                    .current_dir(&dir)
                    .args(a)
                    .status()
                    .unwrap()
                    .success()
            );
        }
        fs::write(dir.join("x"), "a").unwrap();
        git(&dir, &["add", "x"]);
        git(&dir, &["commit", "-m", "i", "-q"]);
        dir
    }

    fn git(repo: &std::path::PathBuf, args: &[&str]) {
        assert!(
            Command::new("git")
                .current_dir(repo)
                .args(args)
                .status()
                .unwrap()
                .success()
        );
    }

    #[test]
    fn commit_message_and_tag_name_build() {
        assert_eq!(
            commit_message("chore(release): v{version}", "1.2.3"),
            "chore(release): v1.2.3"
        );
        assert_eq!(tag_name("v", "1.2.3"), "v1.2.3");
        assert_eq!(tag_name("", "1.2.3"), "1.2.3");
    }

    #[test]
    fn is_clean_detects_dirty() {
        assert!(is_clean("") && is_clean("   \n") && !is_clean(" M src/main.rs") && !is_clean("?? src/main.rs"));
    }

    #[test]
    fn parse_branch_trims_newline() {
        assert!(parse_branch("main\n") == "main" && parse_branch("feature/x") == "feature/x");
    }

    #[test]
    fn stage_dry_run_reports_command_and_empty_returns_none() {
        let report = stage(".", &["a.txt".into(), "b.txt".into()], true).unwrap().unwrap();
        assert!(report.starts_with("git add") && report.contains("a.txt") && report.contains("b.txt"));
        assert!(stage(".", &[], true).unwrap().is_none());
    }

    #[test]
    fn commit_and_tag_dry_run_report_commands() {
        assert_eq!(
            commit(".", "chore: v1.0.0", true).unwrap().unwrap(),
            r#"git commit -m "chore: v1.0.0""#
        );
        assert_eq!(
            tag(".", "v1.0.0", "1.0.0", true).unwrap().unwrap(),
            r#"git tag -a v1.0.0 -m "Release 1.0.0""#
        );
    }

    #[test]
    #[cfg_attr(not(unix), ignore)]
    fn tag_detects_existing_and_skips_at_head() {
        let dir = tmp_repo();
        git(&dir, &["tag", "v1"]);
        assert!(tag_exists(&dir, "v1").unwrap() && !tag_exists(&dir, "v2").unwrap());
        assert_eq!(
            tag(&dir, "v1", "1", false).unwrap(),
            Some("skipped (already points at HEAD): v1".into())
        );
    }

    #[test]
    #[cfg_attr(not(unix), ignore)]
    fn tag_errors_when_points_elsewhere() {
        let dir = tmp_repo();
        let first = rev_parse(&dir, "HEAD").unwrap();
        fs::write(dir.join("x"), "b").unwrap();
        git(&dir, &["commit", "-am", "c2", "-q"]);
        git(&dir, &["tag", "v1", &first]);
        assert!(tag(&dir, "v1", "1", false).is_err());
    }

    #[test]
    #[cfg_attr(not(unix), ignore)]
    fn latest_tag_and_commits_since() {
        let dir = tmp_repo();
        // Initially no tags
        assert_eq!(latest_tag(&dir, None).unwrap(), None);
        assert_eq!(latest_tag(&dir, Some("v")).unwrap(), None);

        // All commits since initial
        let all_commits = commits_since(&dir, None).unwrap();
        assert_eq!(all_commits, vec!["i"]);

        // Tag initial commit as v1.0.0
        git(&dir, &["tag", "v1.0.0"]);
        assert_eq!(latest_tag(&dir, None).unwrap(), Some("v1.0.0".to_string()));
        assert_eq!(latest_tag(&dir, Some("v")).unwrap(), Some("v1.0.0".to_string()));
        assert_eq!(latest_tag(&dir, Some("release/")).unwrap(), None);

        // No commits since v1.0.0 yet
        let since_v1 = commits_since(&dir, Some("v1.0.0")).unwrap();
        assert!(since_v1.is_empty());

        // Add a commit with multiline message
        fs::write(dir.join("x"), "c2").unwrap();
        git(
            &dir,
            &["commit", "-am", "feat: new feature\n\nDetailed explanation", "-q"],
        );

        // Add another commit
        fs::write(dir.join("x"), "c3").unwrap();
        git(&dir, &["commit", "-am", "fix: small bug", "-q"]);

        let new_commits = commits_since(&dir, Some("v1.0.0")).unwrap();
        assert_eq!(new_commits.len(), 2);
        assert_eq!(new_commits[0], "fix: small bug");
        assert_eq!(new_commits[1], "feat: new feature\n\nDetailed explanation");
    }

    #[test]
    fn push_dry_run_formatting() {
        assert_eq!(
            push(".", Some("main"), true, true).unwrap(),
            Some("git push origin main --tags".into())
        );
        assert_eq!(
            push(".", None, true, true).unwrap(),
            Some("git push origin HEAD --tags".into())
        );
        assert_eq!(
            push(".", Some("main"), false, true).unwrap(),
            Some("git push origin main".into())
        );
        assert_eq!(
            push(".", None, false, true).unwrap(),
            Some("git push origin HEAD".into())
        );
    }

    #[test]
    #[cfg_attr(not(unix), ignore)]
    fn push_executes_to_remote() {
        let dir = tmp_repo();
        let remote = std::env::temp_dir().join(tmp_id("cutver-git-remote"));
        let _ = fs::remove_dir_all(&remote);
        fs::create_dir_all(&remote).unwrap();
        assert!(
            Command::new("git")
                .current_dir(&remote)
                .args(["init", "--bare", "-q"])
                .status()
                .unwrap()
                .success()
        );
        git(&dir, &["remote", "add", "origin", remote.to_str().unwrap()]);
        git(&dir, &["tag", "v1.0.0"]);

        assert_eq!(push(&dir, None, true, false).unwrap(), None);

        let out = Command::new("git")
            .current_dir(&remote)
            .args(["tag", "-l", "v1.0.0"])
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "v1.0.0");
        let _ = fs::remove_dir_all(&remote);
    }

    #[test]
    #[cfg_attr(not(unix), ignore)]
    fn status_files_detects_changes() {
        let dir = tmp_repo();
        assert!(status_files(&dir).unwrap().is_empty());
        fs::write(dir.join("x"), "modified").unwrap();
        fs::write(dir.join("y.txt"), "untracked").unwrap();
        let files = status_files(&dir).unwrap();
        assert!(files.contains(&"x".to_string()));
        assert!(files.contains(&"y.txt".to_string()));
    }
}
