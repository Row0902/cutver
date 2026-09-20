#![allow(dead_code)]

static TMP_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn tmp_id(prefix: &str) -> String {
    let n = TMP_SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    format!("{}-{}-{}", prefix, std::process::id(), n)
}

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Mutex;
use std::time::SystemTime;

static CWD_LOCK: Mutex<()> = Mutex::new(());

pub struct Fixture {
    pub dir: PathBuf,
}

impl Fixture {
    pub fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(tmp_id(&format!("cutver-e2e-{name}")));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Self { dir }
    }

    pub fn write(&self, path: impl AsRef<Path>, content: &str) {
        let path = self.dir.join(path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    pub fn read(&self, path: impl AsRef<Path>) -> String {
        fs::read_to_string(self.dir.join(path)).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

pub struct FixtureGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
    prev_dir: PathBuf,
    fixture: Fixture,
}

impl FixtureGuard {
    pub fn new(name: &str) -> Self {
        let lock = CWD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let fixture = Fixture::new(name);
        let prev_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&fixture.dir).unwrap();
        Self {
            _lock: lock,
            prev_dir,
            fixture,
        }
    }

    pub fn fixture(&self) -> &Fixture {
        &self.fixture
    }
}

impl Drop for FixtureGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.prev_dir);
    }
}

pub fn git_available() -> bool {
    Command::new("git")
        .args(["--version"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn run_git(repo: impl AsRef<Path>, args: &[&str]) -> Output {
    Command::new("git")
        .current_dir(repo)
        .args(args)
        .output()
        .expect("git command failed to run")
}

pub fn run_git_ok(repo: impl AsRef<Path>, args: &[&str]) {
    let out = run_git(repo, args);
    assert!(
        out.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&out.stderr)
    );
}

pub fn init_git_repo(fixture: &Fixture) {
    let dir = &fixture.dir;
    run_git_ok(dir, &["init"]);
    run_git_ok(dir, &["config", "user.email", "test@example.com"]);
    run_git_ok(dir, &["config", "user.name", "Test User"]);
    run_git_ok(dir, &["config", "commit.gpgsign", "false"]);
    run_git_ok(dir, &["config", "tag.gpgsign", "false"]);
}

pub fn initial_commit(fixture: &Fixture) {
    run_git_ok(&fixture.dir, &["add", "-A"]);
    run_git_ok(&fixture.dir, &["commit", "-m", "init", "-q"]);
}

pub fn commit_count(fixture: &Fixture) -> usize {
    let out = run_git(&fixture.dir, &["rev-list", "--count", "HEAD"]);
    String::from_utf8_lossy(&out.stdout).trim().parse().unwrap()
}

pub fn tag_exists(fixture: &Fixture, tag: &str) -> bool {
    let out = run_git(&fixture.dir, &["tag", "-l", tag]);
    !String::from_utf8_lossy(&out.stdout).trim().is_empty()
}

pub fn is_annotated_tag(fixture: &Fixture, tag: &str) -> bool {
    let out = run_git(
        &fixture.dir,
        &["for-each-ref", "--format=%(objecttype)", &format!("refs/tags/{tag}")],
    );
    String::from_utf8_lossy(&out.stdout).trim() == "tag"
}

pub fn head_commit_message(fixture: &Fixture) -> String {
    let out = run_git(&fixture.dir, &["log", "-1", "--pretty=%s"]);
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

pub fn head_commit_files(fixture: &Fixture) -> Vec<String> {
    let out = run_git(
        &fixture.dir,
        &["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"],
    );
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

pub const PACKAGE_JSON: &str = r#"{
  "name": "cutver-e2e",
  "version": "1.2.3"
}
"#;

pub const CARGO_TOML: &str = r#"[package]
name = "cutver-e2e"
version = "1.2.3"
edition = "2024"
"#;

pub const GRADLE_KTS: &str = r#"plugins {
    id("com.android.application")
}

android {
    defaultConfig {
        versionName "1.2.3"
        versionCode 42
        applicationId = "com.example.app"
    }
}
"#;

pub const CHANGELOG_MD: &str = r#"# Changelog

All notable changes to this project will be documented in this file.
"#;

pub fn base_cutver_toml(preflight: &str) -> String {
    format!(
        r#"[version]
current_source = "package.json"

[[manifest]]
path = "package.json"
kind = "json"
field = "version"

[[manifest]]
path = "Cargo.toml"
kind = "cargo-package"

[[manifest]]
path = "android/build.gradle.kts"
kind = "gradle"
version_name_field = "versionName"
version_code_field = "versionCode"

{preflight}

[changelog]
path = "CHANGELOG.md"
entry_template = "Maintenance and updates."

[git]
tag_prefix = "v"
require_clean_tree = true
"#
    )
}

pub fn base_release_toml(preflight: &str) -> String {
    base_cutver_toml(preflight)
}

pub fn write_fixture(guard: &FixtureGuard, preflight: &str) {
    let fixture = guard.fixture();
    fixture.write("package.json", PACKAGE_JSON);
    fixture.write("Cargo.toml", CARGO_TOML);
    fixture.write("android/build.gradle.kts", GRADLE_KTS);
    fixture.write("CHANGELOG.md", CHANGELOG_MD);
    fixture.write("cutver.toml", &base_cutver_toml(preflight));
    init_git_repo(fixture);
    initial_commit(fixture);
}

pub fn write_legacy_release_fixture(guard: &FixtureGuard, preflight: &str) {
    let fixture = guard.fixture();
    fixture.write("package.json", PACKAGE_JSON);
    fixture.write("Cargo.toml", CARGO_TOML);
    fixture.write("android/build.gradle.kts", GRADLE_KTS);
    fixture.write("CHANGELOG.md", CHANGELOG_MD);
    fixture.write("release.toml", &base_release_toml(preflight));
    init_git_repo(fixture);
    initial_commit(fixture);
}

pub fn assert_versions_at_123(fixture: &Fixture) {
    assert!(fixture.read("package.json").contains("\"version\": \"1.2.3\""));
    assert!(fixture.read("Cargo.toml").contains("version = \"1.2.3\""));
    let gradle = fixture.read("android/build.gradle.kts");
    assert!(gradle.contains("versionName \"1.2.3\"") && gradle.contains("versionCode 42"));
}

pub fn assert_bumped(fixture: &Fixture) {
    assert!(fixture.read("package.json").contains("\"version\": \"1.3.0\""));
    assert!(fixture.read("Cargo.toml").contains("version = \"1.3.0\""));
    let gradle = fixture.read("android/build.gradle.kts");
    assert!(gradle.contains("versionName \"1.3.0\"") && gradle.contains("versionCode 43"));
    let changelog = fixture.read("CHANGELOG.md");
    let today = cutver::changelog::format_date(SystemTime::now());
    assert!(changelog.contains(&format!("## [v1.3.0] - {today}")));
}
