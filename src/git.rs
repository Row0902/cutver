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
        .map_err(|e| Error::Command { command: args.join(" "), source: e })
}

fn stdout_text(output: Output, command: &str) -> Result<String, Error> {
    if !output.status.success() {
        return Err(Error::Status { command: command.into(), status: output.status });
    }
    String::from_utf8(output.stdout)
        .map_err(|e| Error::Output { source: io::Error::new(io::ErrorKind::InvalidData, e) })
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
        Err(Error::Guard { detail: format!("working tree has uncommitted changes:\n{text}") })
    }
}

pub fn require_branch(repo: impl AsRef<Path>, expected: Option<&str>) -> Result<(), Error> {
    if let Some(branch) = expected {
        let current = stdout_text(run_git(&repo, &["symbolic-ref", "--short", "HEAD"])?, "symbolic-ref --short HEAD")?;
        let current = parse_branch(&current);
        if current != branch {
            return Err(Error::Guard { detail: format!("on branch '{current}', expected '{branch}'") });
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
    let status = Command::new("git").current_dir(&repo).args(&args).status()
        .map_err(|e| Error::Command { command: args.join(" "), source: e })?;
    if status.success() { Ok(None) } else { Err(Error::Status { command: args.join(" "), status }) }
}

pub fn commit(repo: impl AsRef<Path>, message: &str, dry_run: bool) -> Result<Option<String>, Error> {
    if dry_run {
        return Ok(Some(format!(r#"git commit -m "{}""#, message)));
    }
    let status = Command::new("git").current_dir(&repo).args(["commit", "-m", message]).status()
        .map_err(|e| Error::Command { command: "commit".into(), source: e })?;
    if status.success() { Ok(None) } else { Err(Error::Status { command: "commit".into(), status }) }
}

pub fn tag(repo: impl AsRef<Path>, tag_name: &str, version: &str, dry_run: bool) -> Result<Option<String>, Error> {
    let msg = format!("Release {version}");
    if dry_run {
        return Ok(Some(format!(r#"git tag -a {tag_name} -m "{msg}""#)));
    }
    let status = Command::new("git").current_dir(&repo).args(["tag", "-a", tag_name, "-m", &msg]).status()
        .map_err(|e| Error::Command { command: "tag".into(), source: e })?;
    if status.success() { Ok(None) } else { Err(Error::Status { command: "tag".into(), status }) }
}

pub fn commit_message(template: &str, version: &str) -> String {
    template.replace("{version}", version)
}

pub fn tag_name(prefix: &str, version: &str) -> String {
    format!("{prefix}{version}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn commit_message_replaces_version() {
        assert_eq!(commit_message("chore(release): v{version}", "1.2.3"), "chore(release): v1.2.3");
    }

    #[test]
    fn tag_name_builds() {
        assert_eq!(tag_name("v", "1.2.3"), "v1.2.3");
        assert_eq!(tag_name("", "1.2.3"), "1.2.3");
    }

    #[test]
    fn is_clean_detects_dirty() {
        assert!(is_clean(""));
        assert!(is_clean("   \n"));
        assert!(!is_clean(" M src/main.rs"));
        assert!(!is_clean("?? src/main.rs"));
    }

    #[test]
    fn parse_branch_trims_newline() {
        assert_eq!(parse_branch("main\n"), "main");
        assert_eq!(parse_branch("feature/x"), "feature/x");
    }

    #[test]
    fn stage_dry_run_reports_command() {
        let paths = vec!["a.txt".to_string(), "b.txt".to_string()];
        let report = stage(".", &paths, true).unwrap().unwrap();
        assert!(report.starts_with("git add"));
        assert!(report.contains("a.txt"));
        assert!(report.contains("b.txt"));
    }

    #[test]
    fn stage_empty_paths_returns_none() {
        assert!(stage(".", &[], true).unwrap().is_none());
    }

    #[test]
    fn commit_dry_run_reports_command() {
        let report = commit(".", "chore: v1.0.0", true).unwrap().unwrap();
        assert_eq!(report, r#"git commit -m "chore: v1.0.0""#);
    }

    #[test]
    fn tag_dry_run_reports_command() {
        let report = tag(".", "v1.0.0", "1.0.0", true).unwrap().unwrap();
        assert_eq!(report, r#"git tag -a v1.0.0 -m "Release 1.0.0""#);
    }

    #[test]
    #[cfg_attr(not(unix), ignore)]
    fn happy_path_clean_tree_and_branch_guards() {
        let dir = std::env::temp_dir().join(format!("cutver-git-happy-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        for args in [&["init", "-q"][..], &["config", "user.email", "test@example.com"][..], &["config", "user.name", "Test"][..]] {
            assert!(Command::new("git").current_dir(&dir).args(args).status().unwrap().success());
        }
        fs::write(dir.join("x.txt"), "hello").unwrap();
        assert!(Command::new("git").current_dir(&dir).args(["add", "x.txt"]).status().unwrap().success());
        assert!(Command::new("git").current_dir(&dir).args(["commit", "-m", "init", "-q"]).status().unwrap().success());
        require_clean_tree(&dir, true).unwrap();
        require_branch(&dir, Some("main")).unwrap();
        let _ = fs::remove_dir_all(&dir);
    }
}
