#![cfg(unix)]

mod common;

use common::*;
use cutver::bump::run as bump_run;
use cutver::config;
use cutver::semver_bump::Bump;
use std::time::SystemTime;

const PACKAGE_JSON: &str = r#"{
  "name": "cutver-e2e",
  "version": "1.2.3"
}
"#;

const CARGO_TOML: &str = r#"[package]
name = "cutver-e2e"
version = "1.2.3"
edition = "2024"
"#;

const GRADLE_KTS: &str = r#"plugins {
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

const CHANGELOG_MD: &str = r#"# Changelog

All notable changes to this project will be documented in this file.
"#;

fn base_release_toml(preflight: &str) -> String {
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

fn write_fixture(guard: &FixtureGuard, preflight: &str) {
    let fixture = guard.fixture();
    fixture.write("package.json", PACKAGE_JSON);
    fixture.write("Cargo.toml", CARGO_TOML);
    fixture.write("android/build.gradle.kts", GRADLE_KTS);
    fixture.write("CHANGELOG.md", CHANGELOG_MD);
    fixture.write("release.toml", &base_release_toml(preflight));
    init_git_repo(fixture);
    initial_commit(fixture);
}

fn assert_versions_at_123(fixture: &Fixture) {
    assert!(fixture.read("package.json").contains("\"version\": \"1.2.3\""));
    assert!(fixture.read("Cargo.toml").contains("version = \"1.2.3\""));
    let gradle = fixture.read("android/build.gradle.kts");
    assert!(gradle.contains("versionName \"1.2.3\"") && gradle.contains("versionCode 42"));
}

fn assert_bumped(fixture: &Fixture) {
    assert!(fixture.read("package.json").contains("\"version\": \"1.3.0\""));
    assert!(fixture.read("Cargo.toml").contains("version = \"1.3.0\""));
    let gradle = fixture.read("android/build.gradle.kts");
    assert!(gradle.contains("versionName \"1.3.0\"") && gradle.contains("versionCode 43"));
    let changelog = fixture.read("CHANGELOG.md");
    let today = cutver::changelog::format_date(SystemTime::now());
    assert!(changelog.contains(&format!("## [v1.3.0] - {today}")));
    assert!(changelog.contains("Maintenance and updates."));
}

#[test]
fn bump_minor_happy_path() {
    if !git_available() { return; }
    let guard = FixtureGuard::new("happy");
    write_fixture(&guard, r#"[preflight]
check = "true""#);
    let cfg = config::load("release.toml").unwrap();
    let summary = bump_run(&cfg, Bump::Minor, false, &[]).unwrap();
    assert_eq!(summary.next.to_string(), "1.3.0");
    assert!(!summary.dry_run);
    let fixture = guard.fixture();
    assert_bumped(fixture);
    assert_eq!(commit_count(fixture), 2);
    assert_eq!(head_commit_message(fixture), "chore(release): v1.3.0");
    let mut files = head_commit_files(fixture); files.sort();
    assert_eq!(files, vec!["CHANGELOG.md", "Cargo.toml", "android/build.gradle.kts", "package.json"]);
    assert!(tag_exists(fixture, "v1.3.0"));
    assert!(is_annotated_tag(fixture, "v1.3.0"));
}

#[test]
fn bump_and_doctor_from_subdirectory() {
    if !git_available() { return; }
    let guard = FixtureGuard::new("from-subdir");
    write_fixture(&guard, r#"[preflight]
check = "true""#);
    let fixture = guard.fixture();
    let sub = fixture.dir.join("src"); std::fs::create_dir_all(&sub).unwrap(); std::env::set_current_dir(&sub).unwrap();
    let cfg = config::discover(".").unwrap();
    bump_run(&cfg, Bump::Minor, false, &[]).unwrap();
    assert!(fixture.read("package.json").contains("\"version\": \"1.3.0\""));
    assert!(fixture.read("Cargo.toml").contains("version = \"1.3.0\""));
    assert!(cutver::bump::doctor(&cfg).unwrap().is_empty());
}

#[test]
fn bump_aborts_when_preflight_fails() {
    if !git_available() { return; }
    let guard = FixtureGuard::new("preflight-fail");
    write_fixture(&guard, r#"[preflight]
check = "false""#);
    let cfg = config::load("release.toml").unwrap();
    assert!(bump_run(&cfg, Bump::Minor, false, &[]).is_err());
    assert_versions_at_123(guard.fixture());
    assert_eq!(commit_count(guard.fixture()), 1);
    assert!(!tag_exists(guard.fixture(), "v1.3.0"));
}

#[test]
fn bump_dry_run_performs_no_mutation() {
    if !git_available() { return; }
    let guard = FixtureGuard::new("dry-run");
    write_fixture(&guard, r#"[preflight]
check = "true""#);
    let cfg = config::load("release.toml").unwrap();
    let summary = bump_run(&cfg, Bump::Minor, true, &[]).unwrap();
    assert!(summary.dry_run);
    assert_versions_at_123(guard.fixture());
    assert_eq!(commit_count(guard.fixture()), 1);
    assert!(!tag_exists(guard.fixture(), "v1.3.0"));
}

#[test]
fn bump_aborts_when_release_tag_exists_elsewhere() {
    if !git_available() { return; }
    let guard = FixtureGuard::new("tag-conflict");
    write_fixture(&guard, r#"[preflight]
check = "true""#);
    let fixture = guard.fixture();
    fixture.write("extra.txt", "x");
    run_git_ok(&fixture.dir, &["add", "-A"]);
    run_git_ok(&fixture.dir, &["commit", "-m", "second", "-q"]);
    let first = String::from_utf8_lossy(&run_git(&fixture.dir, &["rev-list", "--max-parents=0", "HEAD"]).stdout).trim().to_string();
    run_git_ok(&fixture.dir, &["tag", "v1.3.0", &first]);
    let cfg = config::load("release.toml").unwrap();
    let err = bump_run(&cfg, Bump::Minor, false, &[]).unwrap_err();
    assert!(err.to_string().contains("v1.3.0"));
    assert_versions_at_123(fixture);
    assert_eq!(commit_count(fixture), 2);
    assert!(!tag_exists(fixture, "v1.4.0"));
}

#[test]
fn bump_aborts_when_preflight_times_out() {
    if !git_available() { return; }
    let guard = FixtureGuard::new("preflight-timeout");
    write_fixture(&guard, r#"[preflight]
check = { command = "sleep 30", timeout = 2 }"#);
    let cfg = config::load("release.toml").unwrap();
    let err = bump_run(&cfg, Bump::Minor, false, &[]).unwrap_err();
    assert!(err.to_string().contains("timed out"), "expected timeout error, got {err}");
    assert_versions_at_123(guard.fixture());
    assert_eq!(commit_count(guard.fixture()), 1);
    assert!(!tag_exists(guard.fixture(), "v1.3.0"));
}
