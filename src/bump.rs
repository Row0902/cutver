use crate::preflight;
use semver::Version;
use std::io;
use thiserror::Error;

mod exec;

pub use exec::{doctor, run};

#[derive(Debug, Error)]
pub enum Error {
    #[error("git guard failed: {0}")]
    Git(#[from] crate::git::Error),
    #[error("failed to read '{path}': {source}")]
    Read { path: String, source: io::Error },
    #[error("current source '{path}': {source}")]
    CurrentSource { path: String, source: crate::manifest::Error },
    #[error("manifest '{path}': {source}")]
    Manifest { path: String, source: crate::manifest::Error },
    #[error("preflight failed: {0}")]
    Preflight(#[from] preflight::Error),
    #[error("changelog failed: {0}")]
    Changelog(#[from] crate::changelog::Error),
    #[error("stage failed: {0}")]
    Stage(#[source] crate::git::Error),
    #[error("commit failed: {0}")]
    Commit(#[source] crate::git::Error),
    #[error("tag '{tag}' failed: {source}")]
    Tag { tag: String, #[source] source: crate::git::Error },
    #[error("release tag {tag} already exists (points at {commit}) — resolve it before re-running")]
    TagExists { tag: String, commit: String },
    #[error("write failed for '{path}'; rollback attempted. {rollback}")]
    WriteRollback { path: String, #[source] source: crate::atomic::Error, rollback: String },
}

#[derive(Debug)]
pub struct Touched { pub path: String, pub old: String, pub new: String }

#[derive(Debug)]
pub struct Summary {
    pub source: String,
    pub current: Version,
    pub next: Version,
    pub dry_run: bool,
    pub preflight: Vec<preflight::Step>,
    pub touched: Vec<Touched>,
    pub changelog: Option<String>,
    pub commit_message: String,
    pub tag: String,
    pub tag_skipped: bool,
}

#[derive(Debug)]
pub struct Drift { pub path: String, pub expected: String, pub actual: String }

#[derive(Debug)]
pub(crate) struct Change {
    pub path: String,
    pub old: Version,
    pub original: String,
    pub new: String,
    pub changed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config;
    use crate::semver_bump::Bump;
    use std::fs;

    fn tmp(prefix: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("{}-{}", prefix, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(dir: &std::path::PathBuf, name: &str, text: &str) {
        fs::write(dir.join(name), text).unwrap();
    }

    fn load(dir: &std::path::PathBuf, source: &str, extra: &str) -> config::Config {
        let pkg = dir.join(source).to_string_lossy().to_string();
        let cargo = dir.join("Cargo.toml").to_string_lossy().to_string();
        write(dir, "release.toml", &format!(
            "[version]\ncurrent_source = \"{pkg}\"\n\n[[manifest]]\npath = \"{pkg}\"\nkind = \"json\"\nfield = \"version\"\n\n[[manifest]]\npath = \"{cargo}\"\nkind = \"cargo-package\"\n\n{extra}"
        ));
        config::load(dir.join("release.toml")).unwrap()
    }

    #[test]
    fn doctor_reports_no_drift_when_consistent() {
        let dir = tmp("cutver-doc-ok");
        write(&dir, "package.json", r#"{"version": "1.2.3"}"#);
        write(&dir, "Cargo.toml", "[package]\nversion = \"1.2.3\"\n");
        let drifts = doctor(&load(&dir, "package.json", "")).unwrap();
        assert!(drifts.is_empty());
    }

    #[test]
    fn doctor_reports_drift_when_manifests_differ() {
        let dir = tmp("cutver-doc-drift");
        write(&dir, "package.json", r#"{"version": "1.2.3"}"#);
        write(&dir, "Cargo.toml", "[package]\nversion = \"1.0.0\"\n");
        let drifts = doctor(&load(&dir, "package.json", "")).unwrap();
        assert_eq!(drifts.len(), 1);
        assert_eq!(drifts[0].path, dir.join("Cargo.toml").to_string_lossy().to_string());
        assert_eq!(drifts[0].expected, "1.2.3");
        assert_eq!(drifts[0].actual, "1.0.0");
    }

    #[test]
    fn dry_run_leaves_files_unchanged() {
        let dir = tmp("cutver-bump-dry");
        write(&dir, "package.json", r#"{"version": "1.2.3"}"#);
        write(&dir, "Cargo.toml", "[package]\nversion = \"1.2.3\"\n");
        let summary = run(&load(&dir, "package.json", "[git]\nrequire_clean_tree = false\n"), Bump::Minor, true, &[]).unwrap();
        assert_eq!(summary.next.to_string(), "1.3.0");
        assert!(summary.dry_run);
        assert_eq!(fs::read_to_string(dir.join("Cargo.toml")).unwrap(), "[package]\nversion = \"1.2.3\"\n");
    }

    #[test]
    #[cfg_attr(not(unix), ignore)]
    fn compute_failure_aborts_before_any_write() {
        let dir = tmp("cutver-bump-compute-fail");
        write(&dir, "package.json", r#"{"version": "1.2.3"}"#);
        write(&dir, "bad.json", "not json");
        let pkg = dir.join("package.json").to_string_lossy().to_string();
        let bad = dir.join("bad.json").to_string_lossy().to_string();
        write(&dir, "release.toml", &format!(
            "[version]\ncurrent_source = \"{pkg}\"\n[git]\nrequire_clean_tree = false\n[[manifest]]\npath = \"{pkg}\"\nkind = \"json\"\nfield = \"version\"\n[[manifest]]\npath = \"{bad}\"\nkind = \"json\"\nfield = \"version\"\n"
        ));
        let cfg = config::load(dir.join("release.toml")).unwrap();
        assert!(run(&cfg, Bump::Minor, false, &[]).is_err());
        assert_eq!(fs::read_to_string(dir.join("package.json")).unwrap(), r#"{"version": "1.2.3"}"#);
    }

    #[test]
    #[cfg_attr(not(unix), ignore)]
    fn write_failure_restores_previously_written_files() {
        let dir = tmp("cutver-bump-rollback");
        let a = dir.join("a");
        let b = dir.join("b");
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();
        write(&a, "package.json", r#"{"version": "1.2.3"}"#);
        write(&b, "Cargo.toml", "[package]\nversion = \"1.2.3\"\n");
        let pkg = a.join("package.json").to_string_lossy().to_string();
        let cargo = b.join("Cargo.toml").to_string_lossy().to_string();
        write(&dir, "release.toml", &format!(
            "[version]\ncurrent_source = \"{pkg}\"\n[git]\nrequire_clean_tree = false\n[[manifest]]\npath = \"{pkg}\"\nkind = \"json\"\nfield = \"version\"\n[[manifest]]\npath = \"{cargo}\"\nkind = \"cargo-package\"\n"
        ));
        let mut perms = fs::metadata(&b).unwrap().permissions();
        perms.set_readonly(true);
        fs::set_permissions(&b, perms).unwrap();
        let cfg = config::load(dir.join("release.toml")).unwrap();
        assert!(run(&cfg, Bump::Minor, false, &[]).is_err());
        assert_eq!(fs::read_to_string(a.join("package.json")).unwrap(), r#"{"version": "1.2.3"}"#);
        let mut perms = fs::metadata(&b).unwrap().permissions();
        perms.set_readonly(false);
        let _ = fs::set_permissions(&b, perms);
    }
}
