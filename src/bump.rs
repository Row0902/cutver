use crate::preflight;
use semver::Version;
use std::io;
use thiserror::Error;

mod exec;

pub use exec::{doctor, doctor_changelog, run, run_with_first_release};

#[derive(Debug, Error)]
pub enum Error {
    #[error("git guard failed: {0}")]
    Git(#[from] crate::git::Error),
    #[error("failed to read '{path}': {source}")]
    Read { path: String, source: io::Error },
    #[error("current source '{path}': {source}")]
    CurrentSource {
        path: String,
        source: crate::manifest::Error,
    },
    #[error("manifest '{path}': {source}")]
    Manifest {
        path: String,
        source: crate::manifest::Error,
    },
    #[error("preflight failed: {0}")]
    Preflight(#[from] preflight::Error),
    #[error("changelog failed: {0}")]
    Changelog(#[from] crate::changelog::Error),
    #[error("stage failed: {0}")]
    Stage(#[source] crate::git::Error),
    #[error("commit failed: {0}")]
    Commit(#[source] crate::git::Error),
    #[error("tag '{tag}' failed: {source}")]
    Tag {
        tag: String,
        #[source]
        source: crate::git::Error,
    },
    #[error("release tag {tag} already exists (points at {commit}) — resolve it before re-running")]
    TagExists { tag: String, commit: String },
    #[error("write failed for '{path}'; rollback attempted. {rollback}")]
    WriteRollback {
        path: String,
        #[source]
        source: crate::atomic::Error,
        rollback: String,
    },
    #[error("post_bump hook '{command}' failed with status {status}")]
    PostBumpHookFailed {
        command: String,
        status: std::process::ExitStatus,
    },
    #[error("failed to run post_bump hook '{command}': {source}")]
    PostBumpHookSpawn {
        command: String,
        #[source]
        source: io::Error,
    },
    #[error("publish push failed: {0}")]
    Push(#[source] crate::git::Error),
    #[error("publish command '{command}' failed with status {status}")]
    PublishCommandFailed {
        command: String,
        status: std::process::ExitStatus,
    },
    #[error("failed to run publish command '{command}': {source}")]
    PublishCommandSpawn {
        command: String,
        #[source]
        source: io::Error,
    },
    #[error("publish command '{command}' timed out after {elapsed_ms}ms (limit {timeout}s)")]
    PublishCommandTimeout {
        command: String,
        timeout: u64,
        elapsed_ms: u128,
    },
}

#[derive(Debug)]
pub struct Touched {
    pub path: String,
    pub old: String,
    pub new: String,
}

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
    pub post_bump: Option<String>,
    pub publish_push: bool,
    pub publish_push_command: Option<String>,
    pub publish_commands: Vec<String>,
}

#[derive(Debug)]
pub struct Drift {
    pub path: String,
    pub expected: String,
    pub actual: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChangelogDrift {
    pub missing_in_changelog: Vec<String>,
    pub orphan_sections: Vec<String>,
}

impl ChangelogDrift {
    pub fn is_empty(&self) -> bool {
        self.missing_in_changelog.is_empty() && self.orphan_sections.is_empty()
    }
}

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
    static TMP_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    fn tmp_id(prefix: &str) -> String {
        let n = TMP_SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        format!("{}-{}-{}", prefix, std::process::id(), n)
    }

    use super::*;
    use crate::cli::BumpLevel;
    use crate::config;
    use crate::semver_bump::Bump;
    use std::fs;
    use std::path::Path;

    fn tmp(prefix: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(tmp_id(prefix));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(dir: &std::path::Path, name: &str, text: &str) {
        fs::write(dir.join(name), text).unwrap();
    }

    fn load(dir: &std::path::Path, source: &str, extra: &str) -> config::Config {
        write(
            dir,
            "release.toml",
            &format!(
                "[version]\ncurrent_source = \"{source}\"\n\n[[manifest]]\npath = \"{source}\"\nkind = \"json\"\nfield = \"version\"\n\n[[manifest]]\npath = \"Cargo.toml\"\nkind = \"cargo-package\"\n\n{extra}"
            ),
        );
        config::load(dir.join("release.toml")).unwrap()
    }

    fn git_commit(dir: &Path, msg: &str) {
        let status = std::process::Command::new("git")
            .current_dir(dir)
            .args(["add", "."])
            .status()
            .unwrap();
        assert!(status.success(), "git add failed");
        let status = std::process::Command::new("git")
            .current_dir(dir)
            .args(["commit", "-m", msg, "-q"])
            .status()
            .unwrap();
        assert!(status.success(), "git commit failed");
    }

    fn git_tag(dir: &Path, tag: &str) {
        let status = std::process::Command::new("git")
            .current_dir(dir)
            .args(["tag", tag])
            .status()
            .unwrap();
        assert!(status.success(), "git tag failed: {tag}");
    }

    #[test]
    fn test_changelog_drift_is_empty() {
        let mut drift = ChangelogDrift::default();
        assert!(drift.is_empty());
        drift.missing_in_changelog.push("v1.0.0".into());
        assert!(!drift.is_empty());
        drift.missing_in_changelog.clear();
        drift.orphan_sections.push("1.0.0".into());
        assert!(!drift.is_empty());
    }

    #[test]
    fn test_doctor_changelog_consistent() {
        let dir = tmp("cutver-doc-cl-ok");
        crate::git::init_test_repo(&dir);
        write(&dir, "package.json", r#"{"version": "1.1.0"}"#);
        write(&dir, "Cargo.toml", "[package]\nversion = \"1.1.0\"\n");
        write(
            &dir,
            "CHANGELOG.md",
            "# Changelog\n\n## [1.1.0] - 2024-01-02\n- feature\n\n## [1.0.0] - 2024-01-01\n- initial\n",
        );
        git_commit(&dir, "chore: initial");
        git_tag(&dir, "v1.0.0");
        git_tag(&dir, "v1.1.0");

        let cfg = load(&dir, "package.json", "");
        let drift = doctor_changelog(&cfg).unwrap();
        assert!(drift.is_empty());
        assert!(drift.missing_in_changelog.is_empty());
        assert!(drift.orphan_sections.is_empty());
    }

    #[test]
    fn test_doctor_changelog_missing_in_changelog() {
        let dir = tmp("cutver-doc-cl-missing");
        crate::git::init_test_repo(&dir);
        write(&dir, "package.json", r#"{"version": "1.1.0"}"#);
        write(&dir, "Cargo.toml", "[package]\nversion = \"1.1.0\"\n");
        write(
            &dir,
            "CHANGELOG.md",
            "# Changelog\n\n## [1.0.0] - 2024-01-01\n- initial\n",
        );
        git_commit(&dir, "chore: initial");
        git_tag(&dir, "v1.0.0");
        git_tag(&dir, "v1.1.0");

        let cfg = load(&dir, "package.json", "");
        let drift = doctor_changelog(&cfg).unwrap();
        assert!(!drift.is_empty());
        assert_eq!(drift.missing_in_changelog, vec!["v1.1.0"]);
        assert!(drift.orphan_sections.is_empty());
    }

    #[test]
    fn test_doctor_changelog_orphan_section() {
        let dir = tmp("cutver-doc-cl-orphan");
        crate::git::init_test_repo(&dir);
        write(&dir, "package.json", r#"{"version": "1.0.0"}"#);
        write(&dir, "Cargo.toml", "[package]\nversion = \"1.0.0\"\n");
        write(
            &dir,
            "CHANGELOG.md",
            "# Changelog\n\n## [1.2.0] - 2024-01-03\n- unreleased\n\n## [1.0.0] - 2024-01-01\n- initial\n",
        );
        git_commit(&dir, "chore: initial");
        git_tag(&dir, "v1.0.0");

        let cfg = load(&dir, "package.json", "");
        let drift = doctor_changelog(&cfg).unwrap();
        assert!(!drift.is_empty());
        assert!(drift.missing_in_changelog.is_empty());
        assert_eq!(drift.orphan_sections, vec!["1.2.0"]);
    }

    #[test]
    fn test_doctor_changelog_custom_prefix_and_path() {
        let dir = tmp("cutver-doc-cl-custom");
        crate::git::init_test_repo(&dir);
        fs::create_dir_all(dir.join("docs")).unwrap();
        write(&dir, "package.json", r#"{"version": "1.0.0"}"#);
        write(&dir, "Cargo.toml", "[package]\nversion = \"1.0.0\"\n");
        write(
            &dir,
            "docs/HISTORY.md",
            "# History\n\n## [1.0.0] - 2024-01-01\n- initial\n",
        );
        git_commit(&dir, "chore: initial");
        git_tag(&dir, "release/1.0.0");
        git_tag(&dir, "release/1.1.0");

        let cfg = load(
            &dir,
            "package.json",
            "[git]\ntag_prefix = \"release/\"\n\n[changelog]\npath = \"docs/HISTORY.md\"\n",
        );
        let drift = doctor_changelog(&cfg).unwrap();
        assert!(!drift.is_empty());
        assert_eq!(drift.missing_in_changelog, vec!["release/1.1.0"]);
        assert!(drift.orphan_sections.is_empty());
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
        assert_eq!(Path::new(&drifts[0].path).file_name().unwrap(), "Cargo.toml");
        assert_eq!(drifts[0].expected, "1.2.3");
        assert_eq!(drifts[0].actual, "1.0.0");
    }

    #[test]
    fn dry_run_leaves_files_unchanged() {
        let dir = tmp("cutver-bump-dry");
        crate::git::init_test_repo(&dir);
        write(&dir, "package.json", r#"{"version": "1.2.3"}"#);
        write(&dir, "Cargo.toml", "[package]\nversion = \"1.2.3\"\n");
        let summary = run(
            &load(&dir, "package.json", "[git]\nrequire_clean_tree = false\n"),
            Bump::Minor,
            true,
            &[],
        )
        .unwrap();
        assert_eq!(summary.next.to_string(), "1.3.0");
        assert!(summary.dry_run);
        assert_eq!(
            fs::read_to_string(dir.join("Cargo.toml")).unwrap(),
            "[package]\nversion = \"1.2.3\"\n"
        );
    }

    #[test]
    fn dry_run_auto_bump_deduces_from_commits() {
        let dir = tmp("cutver-bump-dry-auto");
        crate::git::init_test_repo(&dir);
        write(&dir, "package.json", r#"{"version": "1.2.3"}"#);
        write(&dir, "Cargo.toml", "[package]\nversion = \"1.2.3\"\n");
        // Create an initial commit and tag it as v1.2.3
        std::process::Command::new("git")
            .current_dir(&dir)
            .args(["add", "package.json", "Cargo.toml"])
            .status()
            .unwrap();
        std::process::Command::new("git")
            .current_dir(&dir)
            .args(["commit", "-m", "chore: initial release", "-q"])
            .status()
            .unwrap();
        std::process::Command::new("git")
            .current_dir(&dir)
            .args(["tag", "v1.2.3"])
            .status()
            .unwrap();
        // Add a feat commit after the tag
        write(&dir, "feature.txt", "feature content");
        std::process::Command::new("git")
            .current_dir(&dir)
            .args(["add", "feature.txt"])
            .status()
            .unwrap();
        std::process::Command::new("git")
            .current_dir(&dir)
            .args(["commit", "-m", "feat: add brilliant feature", "-q"])
            .status()
            .unwrap();

        let cfg = load(&dir, "package.json", "[git]\nrequire_clean_tree = false\n");
        let summary = run(&cfg, BumpLevel::Auto, true, &[]).unwrap();
        assert_eq!(summary.next.to_string(), "1.3.0");
        assert!(summary.dry_run);
    }

    #[test]
    #[cfg_attr(not(unix), ignore)]
    fn compute_failure_aborts_before_any_write() {
        let dir = tmp("cutver-bump-compute-fail");
        write(&dir, "package.json", r#"{"version": "1.2.3"}"#);
        write(&dir, "bad.json", "not json");
        let pkg = dir.join("package.json").to_string_lossy().to_string();
        let bad = dir.join("bad.json").to_string_lossy().to_string();
        write(
            &dir,
            "release.toml",
            &format!(
                "[version]\ncurrent_source = \"{pkg}\"\n[git]\nrequire_clean_tree = false\n[[manifest]]\npath = \"{pkg}\"\nkind = \"json\"\nfield = \"version\"\n[[manifest]]\npath = \"{bad}\"\nkind = \"json\"\nfield = \"version\"\n"
            ),
        );
        let cfg = config::load(dir.join("release.toml")).unwrap();
        assert!(run(&cfg, Bump::Minor, false, &[]).is_err());
        assert_eq!(
            fs::read_to_string(dir.join("package.json")).unwrap(),
            r#"{"version": "1.2.3"}"#
        );
    }

    #[test]
    #[cfg_attr(not(unix), ignore)]
    #[allow(clippy::permissions_set_readonly_false)]
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
        write(
            &dir,
            "release.toml",
            &format!(
                "[version]\ncurrent_source = \"{pkg}\"\n[git]\nrequire_clean_tree = false\n[[manifest]]\npath = \"{pkg}\"\nkind = \"json\"\nfield = \"version\"\n[[manifest]]\npath = \"{cargo}\"\nkind = \"cargo-package\"\n"
            ),
        );
        let mut perms = fs::metadata(&b).unwrap().permissions();
        perms.set_readonly(true);
        fs::set_permissions(&b, perms).unwrap();
        let cfg = config::load(dir.join("release.toml")).unwrap();
        assert!(run(&cfg, Bump::Minor, false, &[]).is_err());
        assert_eq!(
            fs::read_to_string(a.join("package.json")).unwrap(),
            r#"{"version": "1.2.3"}"#
        );
        let mut perms = fs::metadata(&b).unwrap().permissions();
        perms.set_readonly(false);
        let _ = fs::set_permissions(&b, perms);
    }
}
