#![cfg(unix)]

mod common;

use common::*;
use cutver::bump::run as bump_run;
use cutver::cli::BumpLevel;
use cutver::config;
use cutver::semver_bump::Bump;
use std::process::Command;
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

fn base_cutver_toml(preflight: &str) -> String {
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

fn base_release_toml(preflight: &str) -> String {
    base_cutver_toml(preflight)
}

fn write_fixture(guard: &FixtureGuard, preflight: &str) {
    let fixture = guard.fixture();
    fixture.write("package.json", PACKAGE_JSON);
    fixture.write("Cargo.toml", CARGO_TOML);
    fixture.write("android/build.gradle.kts", GRADLE_KTS);
    fixture.write("CHANGELOG.md", CHANGELOG_MD);
    fixture.write("cutver.toml", &base_cutver_toml(preflight));
    init_git_repo(fixture);
    initial_commit(fixture);
}

fn write_legacy_release_fixture(guard: &FixtureGuard, preflight: &str) {
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
}

#[test]
fn bump_minor_happy_path() {
    assert!(
        git_available(),
        "git CLI is required for e2e tests but was not found in PATH"
    );
    let guard = FixtureGuard::new("happy");
    write_fixture(
        &guard,
        r#"[preflight]
check = "true""#,
    );
    let cfg = config::load("cutver.toml").unwrap();
    let summary = bump_run(&cfg, Bump::Minor, false, &[]).unwrap();
    assert_eq!(summary.next.to_string(), "1.3.0");
    assert!(!summary.dry_run);
    let fixture = guard.fixture();
    assert_bumped(fixture);
    assert_eq!(commit_count(fixture), 2);
    assert_eq!(head_commit_message(fixture), "chore(release): v1.3.0");
    let mut files = head_commit_files(fixture);
    files.sort();
    assert_eq!(
        files,
        vec!["CHANGELOG.md", "Cargo.toml", "android/build.gradle.kts", "package.json"]
    );
    assert!(tag_exists(fixture, "v1.3.0"));
    assert!(is_annotated_tag(fixture, "v1.3.0"));
}

#[test]
fn bump_and_doctor_from_subdirectory() {
    assert!(
        git_available(),
        "git CLI is required for e2e tests but was not found in PATH"
    );
    let guard = FixtureGuard::new("from-subdir");
    write_fixture(
        &guard,
        r#"[preflight]
check = "true""#,
    );
    let fixture = guard.fixture();
    let sub = fixture.dir.join("src");
    std::fs::create_dir_all(&sub).unwrap();
    std::env::set_current_dir(&sub).unwrap();
    let cfg = config::discover(".").unwrap();
    bump_run(&cfg, Bump::Minor, false, &[]).unwrap();
    assert!(fixture.read("package.json").contains("\"version\": \"1.3.0\""));
    assert!(fixture.read("Cargo.toml").contains("version = \"1.3.0\""));
    assert!(cutver::bump::doctor(&cfg).unwrap().is_empty());
}

#[test]
fn bump_aborts_when_preflight_fails() {
    assert!(
        git_available(),
        "git CLI is required for e2e tests but was not found in PATH"
    );
    let guard = FixtureGuard::new("preflight-fail");
    write_fixture(
        &guard,
        r#"[preflight]
check = "false""#,
    );
    let cfg = config::load("cutver.toml").unwrap();
    assert!(bump_run(&cfg, Bump::Minor, false, &[]).is_err());
    assert_versions_at_123(guard.fixture());
    assert_eq!(commit_count(guard.fixture()), 1);
    assert!(!tag_exists(guard.fixture(), "v1.3.0"));
}

#[test]
fn bump_dry_run_performs_no_mutation() {
    assert!(
        git_available(),
        "git CLI is required for e2e tests but was not found in PATH"
    );
    let guard = FixtureGuard::new("dry-run");
    write_fixture(
        &guard,
        r#"[preflight]
check = "true""#,
    );
    let cfg = config::load("cutver.toml").unwrap();
    let summary = bump_run(&cfg, Bump::Minor, true, &[]).unwrap();
    assert!(summary.dry_run);
    assert_versions_at_123(guard.fixture());
    assert_eq!(commit_count(guard.fixture()), 1);
    assert!(!tag_exists(guard.fixture(), "v1.3.0"));
}

#[test]
fn bump_aborts_when_release_tag_exists_elsewhere() {
    assert!(
        git_available(),
        "git CLI is required for e2e tests but was not found in PATH"
    );
    let guard = FixtureGuard::new("tag-conflict");
    write_fixture(
        &guard,
        r#"[preflight]
check = "true""#,
    );
    let fixture = guard.fixture();
    fixture.write("extra.txt", "x");
    run_git_ok(&fixture.dir, &["add", "-A"]);
    run_git_ok(&fixture.dir, &["commit", "-m", "second", "-q"]);
    let first = String::from_utf8_lossy(&run_git(&fixture.dir, &["rev-list", "--max-parents=0", "HEAD"]).stdout)
        .trim()
        .to_string();
    run_git_ok(&fixture.dir, &["tag", "v1.3.0", &first]);
    let cfg = config::load("cutver.toml").unwrap();
    let err = bump_run(&cfg, Bump::Minor, false, &[]).unwrap_err();
    assert!(err.to_string().contains("v1.3.0"));
    assert_versions_at_123(fixture);
    assert_eq!(commit_count(fixture), 2);
    assert!(!tag_exists(fixture, "v1.4.0"));
}

#[test]
fn bump_aborts_when_preflight_times_out() {
    assert!(
        git_available(),
        "git CLI is required for e2e tests but was not found in PATH"
    );
    let guard = FixtureGuard::new("preflight-timeout");
    write_fixture(
        &guard,
        r#"[preflight]
check = { command = "sleep 30", timeout = 2 }"#,
    );
    let cfg = config::load("cutver.toml").unwrap();
    let err = bump_run(&cfg, Bump::Minor, false, &[]).unwrap_err();
    assert!(
        err.to_string().contains("timed out"),
        "expected timeout error, got {err}"
    );
    assert_versions_at_123(guard.fixture());
    assert_eq!(commit_count(guard.fixture()), 1);
    assert!(!tag_exists(guard.fixture(), "v1.3.0"));
}

#[test]
fn bump_dry_run_aborts_when_release_tag_exists() {
    assert!(
        git_available(),
        "git CLI is required for e2e tests but was not found in PATH"
    );
    let guard = FixtureGuard::new("dry-run-tag-conflict");
    write_fixture(
        &guard,
        r#"[preflight]
check = "true""#,
    );
    let fixture = guard.fixture();
    run_git_ok(&fixture.dir, &["tag", "v1.3.0", "HEAD"]);
    let cfg = config::load("cutver.toml").unwrap();
    let err = bump_run(&cfg, Bump::Minor, true, &[]).unwrap_err();
    assert!(
        matches!(err, cutver::bump::Error::TagExists { .. }),
        "expected TagExists error, got {err:?}"
    );
    assert!(err.to_string().contains("v1.3.0"));
    assert_versions_at_123(fixture);
    assert_eq!(commit_count(fixture), 1);
}

#[test]
#[cfg_attr(not(unix), ignore)]
fn bump_rolls_back_manifests_when_commit_fails() {
    use std::os::unix::fs::PermissionsExt;

    assert!(
        git_available(),
        "git CLI is required for e2e tests but was not found in PATH"
    );
    let guard = FixtureGuard::new("commit-fail-rollback");
    write_fixture(
        &guard,
        r#"[preflight]
check = "true""#,
    );
    let fixture = guard.fixture();

    let hook_path = fixture.dir.join(".git/hooks/pre-commit");
    std::fs::write(&hook_path, "#!/bin/sh\nexit 1\n").unwrap();
    std::fs::set_permissions(&hook_path, std::fs::Permissions::from_mode(0o755)).unwrap();

    let cfg = config::load("cutver.toml").unwrap();
    let err = bump_run(&cfg, Bump::Minor, false, &[]).unwrap_err();
    assert!(
        matches!(err, cutver::bump::Error::Commit(_)),
        "expected Commit error, got {err:?}"
    );
    assert_versions_at_123(fixture);
    assert_eq!(commit_count(fixture), 1);
    assert!(!tag_exists(fixture, "v1.3.0"));
    let status = run_git(&fixture.dir, &["status", "--porcelain"]);
    assert!(
        status.stdout.is_empty(),
        "expected empty git status --porcelain, got: {}",
        String::from_utf8_lossy(&status.stdout)
    );
}

#[test]
fn bump_rolls_back_manifests_when_stage_fails() {
    assert!(
        git_available(),
        "git CLI is required for e2e tests but was not found in PATH"
    );
    let guard = FixtureGuard::new("stage-fail-rollback");
    write_fixture(
        &guard,
        r#"[preflight]
check = "touch .git/index.lock""#,
    );
    let fixture = guard.fixture();
    let cfg = config::load("cutver.toml").unwrap();
    let err = bump_run(&cfg, Bump::Minor, false, &[]).unwrap_err();
    assert!(
        matches!(err, cutver::bump::Error::Stage(_)),
        "expected Stage error, got {err:?}"
    );

    let _ = std::fs::remove_file(fixture.dir.join(".git/index.lock"));
    assert_versions_at_123(fixture);
    assert_eq!(commit_count(fixture), 1);
    assert!(!tag_exists(fixture, "v1.3.0"));
}

#[test]
fn discover_stops_at_git_boundary_without_release_toml() {
    assert!(
        git_available(),
        "git CLI is required for e2e tests but was not found in PATH"
    );
    let guard = FixtureGuard::new("boundary-stop");
    let fixture = guard.fixture();
    fixture.write(
        "release.toml",
        &base_release_toml(
            r#"[preflight]
check = "true""#,
        ),
    );
    fixture.write("package.json", PACKAGE_JSON);
    fixture.write("Cargo.toml", CARGO_TOML);
    fixture.write("android/build.gradle.kts", GRADLE_KTS);
    fixture.write("CHANGELOG.md", CHANGELOG_MD);

    let inner_repo = fixture.dir.join("inner_repo");
    std::fs::create_dir_all(&inner_repo).unwrap();
    run_git_ok(&inner_repo, &["init"]);
    let sub = inner_repo.join("src").join("nested");
    std::fs::create_dir_all(&sub).unwrap();

    let err = config::discover(&sub).unwrap_err();
    assert!(
        matches!(err, config::ConfigError::NotFound(_)),
        "expected NotFound error stopping at .git boundary, got {err:?}"
    );
}

#[test]
fn discover_from_deep_subdirectory_finds_release_toml() {
    assert!(
        git_available(),
        "git CLI is required for e2e tests but was not found in PATH"
    );
    let guard = FixtureGuard::new("deep-subdir-discover");
    write_legacy_release_fixture(
        &guard,
        r#"[preflight]
check = "true""#,
    );
    let fixture = guard.fixture();
    let deep = fixture.dir.join("nested").join("deep").join("child");
    std::fs::create_dir_all(&deep).unwrap();
    let cfg = config::discover(&deep).unwrap();
    let expected_root = std::fs::canonicalize(&fixture.dir).unwrap();
    assert_eq!(cfg.root_dir, expected_root);
}

#[test]
fn bump_minor_with_canonical_cutver_toml_deduction() {
    assert!(
        git_available(),
        "git CLI is required for e2e tests but was not found in PATH"
    );
    let guard = FixtureGuard::new("cutver-deduction");
    let fixture = guard.fixture();
    fixture.write("package.json", PACKAGE_JSON);
    fixture.write("Cargo.toml", CARGO_TOML);
    fixture.write("android/build.gradle.kts", GRADLE_KTS);
    fixture.write("CHANGELOG.md", CHANGELOG_MD);
    fixture.write(
        "cutver.toml",
        r#"[[manifest]]
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

[preflight]
check = "true"

[changelog]
path = "CHANGELOG.md"
entry_template = "Maintenance and updates."

[git]
tag_prefix = "v"
require_clean_tree = true
"#,
    );
    init_git_repo(fixture);
    initial_commit(fixture);

    let cfg = config::discover(&fixture.dir).unwrap();
    assert!(cfg.version.current_source.ends_with("package.json"));
    let summary = bump_run(&cfg, Bump::Minor, false, &[]).unwrap();
    assert_eq!(summary.next.to_string(), "1.3.0");
    assert_bumped(fixture);
}

#[test]
fn discover_prioritizes_cutver_toml_over_release_toml_e2e() {
    assert!(
        git_available(),
        "git CLI is required for e2e tests but was not found in PATH"
    );
    let guard = FixtureGuard::new("precedence-e2e");
    let fixture = guard.fixture();
    fixture.write("package.json", PACKAGE_JSON);
    fixture.write("Cargo.toml", CARGO_TOML);
    fixture.write("android/build.gradle.kts", GRADLE_KTS);
    fixture.write("CHANGELOG.md", CHANGELOG_MD);
    // release.toml uses package.json
    fixture.write(
        "release.toml",
        r#"[version]
current_source = "package.json"
[[manifest]]
path = "package.json"
kind = "json"
field = "version"
"#,
    );
    // cutver.toml uses Cargo.toml
    fixture.write(
        "cutver.toml",
        r#"[[manifest]]
path = "Cargo.toml"
kind = "cargo-package"
"#,
    );
    init_git_repo(fixture);
    initial_commit(fixture);

    let sub = fixture.dir.join("nested");
    std::fs::create_dir_all(&sub).unwrap();
    let cfg = config::discover(&sub).unwrap();
    assert!(cfg.version.current_source.ends_with("Cargo.toml"));
}

#[test]
fn bump_minor_with_legacy_release_toml_fallback() {
    assert!(
        git_available(),
        "git CLI is required for e2e tests but was not found in PATH"
    );
    let guard = FixtureGuard::new("legacy-fallback");
    write_legacy_release_fixture(
        &guard,
        r#"[preflight]
check = "true""#,
    );
    let cfg = config::load("release.toml").unwrap();
    let summary = bump_run(&cfg, Bump::Minor, false, &[]).unwrap();
    assert_eq!(summary.next.to_string(), "1.3.0");
    assert!(!summary.dry_run);
    assert_bumped(guard.fixture());
}

#[test]
fn bump_auto_dry_run_with_feat() {
    assert!(
        git_available(),
        "git CLI is required for e2e tests but was not found in PATH"
    );
    let guard = FixtureGuard::new("auto-feat-dry");
    write_fixture(
        &guard,
        r#"[preflight]
check = "true""#,
    );
    let fixture = guard.fixture();
    run_git_ok(
        &fixture.dir,
        &["commit", "--allow-empty", "-m", "feat: add auto bump support"],
    );

    let cfg = config::load("cutver.toml").unwrap();
    let summary = bump_run(&cfg, BumpLevel::Auto, true, &[]).unwrap();
    assert_eq!(summary.next.to_string(), "1.3.0");
    assert!(summary.dry_run);
    assert_versions_at_123(fixture);
    assert!(!tag_exists(fixture, "v1.3.0"));
}

#[test]
fn bump_auto_real_run_with_feat() {
    assert!(
        git_available(),
        "git CLI is required for e2e tests but was not found in PATH"
    );
    let guard = FixtureGuard::new("auto-feat-real");
    write_fixture(
        &guard,
        r#"[preflight]
check = "true""#,
    );
    let fixture = guard.fixture();
    run_git_ok(
        &fixture.dir,
        &["commit", "--allow-empty", "-m", "feat: add auto bump support"],
    );

    let cfg = config::load("cutver.toml").unwrap();
    let summary = bump_run(&cfg, BumpLevel::Auto, false, &[]).unwrap();
    assert_eq!(summary.next.to_string(), "1.3.0");
    assert!(!summary.dry_run);
    assert_bumped(fixture);
    let cl = fixture.read("CHANGELOG.md");
    assert!(cl.contains("### Features\n- add auto bump support"));
    assert!(tag_exists(fixture, "v1.3.0"));
    assert!(is_annotated_tag(fixture, "v1.3.0"));
}

#[test]
fn bump_auto_real_run_with_breaking_exclamation() {
    assert!(
        git_available(),
        "git CLI is required for e2e tests but was not found in PATH"
    );
    let guard = FixtureGuard::new("auto-break-excl");
    write_fixture(
        &guard,
        r#"[preflight]
check = "true""#,
    );
    let fixture = guard.fixture();
    run_git_ok(
        &fixture.dir,
        &["commit", "--allow-empty", "-m", "feat!: breaking change across system"],
    );

    let cfg = config::load("cutver.toml").unwrap();
    let summary = bump_run(&cfg, BumpLevel::Auto, false, &[]).unwrap();
    assert_eq!(summary.next.to_string(), "2.0.0");
    assert!(!summary.dry_run);
    assert!(fixture.read("package.json").contains("\"version\": \"2.0.0\""));
    assert!(fixture.read("Cargo.toml").contains("version = \"2.0.0\""));
    assert!(tag_exists(fixture, "v2.0.0"));
}

#[test]
fn bump_auto_real_run_with_breaking_footer() {
    assert!(
        git_available(),
        "git CLI is required for e2e tests but was not found in PATH"
    );
    let guard = FixtureGuard::new("auto-break-footer");
    write_fixture(
        &guard,
        r#"[preflight]
check = "true""#,
    );
    let fixture = guard.fixture();
    run_git_ok(
        &fixture.dir,
        &[
            "commit",
            "--allow-empty",
            "-m",
            "fix: correct edge case in config\n\nBREAKING CHANGE: drop deprecated release.toml field",
        ],
    );

    let cfg = config::load("cutver.toml").unwrap();
    let summary = bump_run(&cfg, BumpLevel::Auto, false, &[]).unwrap();
    assert_eq!(summary.next.to_string(), "2.0.0");
    assert!(!summary.dry_run);
    assert!(fixture.read("package.json").contains("\"version\": \"2.0.0\""));
    assert!(fixture.read("Cargo.toml").contains("version = \"2.0.0\""));
    assert!(tag_exists(fixture, "v2.0.0"));
}

#[test]
fn bump_auto_defaults_to_patch_for_fixes_and_non_conventional() {
    assert!(
        git_available(),
        "git CLI is required for e2e tests but was not found in PATH"
    );
    let guard = FixtureGuard::new("auto-patch-fixes");
    write_fixture(
        &guard,
        r#"[preflight]
check = "true""#,
    );
    let fixture = guard.fixture();
    run_git_ok(&fixture.dir, &["commit", "--allow-empty", "-m", "fix: minor bug fix"]);
    run_git_ok(&fixture.dir, &["commit", "--allow-empty", "-m", "docs: update readme"]);
    run_git_ok(
        &fixture.dir,
        &["commit", "--allow-empty", "-m", "some non-conventional commit"],
    );

    let cfg = config::load("cutver.toml").unwrap();
    let summary = bump_run(&cfg, BumpLevel::Auto, false, &[]).unwrap();
    assert_eq!(summary.next.to_string(), "1.2.4");
    assert!(!summary.dry_run);
    assert!(fixture.read("package.json").contains("\"version\": \"1.2.4\""));
    assert!(fixture.read("Cargo.toml").contains("version = \"1.2.4\""));
    assert!(tag_exists(fixture, "v1.2.4"));
}

#[test]
fn bump_auto_respects_previous_tag() {
    assert!(
        git_available(),
        "git CLI is required for e2e tests but was not found in PATH"
    );
    let guard = FixtureGuard::new("auto-respect-tag");
    write_fixture(
        &guard,
        r#"[preflight]
check = "true""#,
    );
    let fixture = guard.fixture();
    // Prior breaking commit before tagging v1.2.3
    run_git_ok(
        &fixture.dir,
        &[
            "commit",
            "--allow-empty",
            "-m",
            "feat!: breaking change prior to release",
        ],
    );
    run_git_ok(&fixture.dir, &["tag", "v1.2.3"]);

    // Subsequent fix commit after v1.2.3 tag
    run_git_ok(
        &fixture.dir,
        &["commit", "--allow-empty", "-m", "fix: post-release bugfix"],
    );

    let cfg = config::load("cutver.toml").unwrap();
    let summary = bump_run(&cfg, BumpLevel::Auto, false, &[]).unwrap();
    // Because the breaking change occurred before v1.2.3 tag, it only inspects commits since v1.2.3,
    // deducing a patch bump to 1.2.4 instead of major.
    assert_eq!(summary.next.to_string(), "1.2.4");
    assert!(!summary.dry_run);
    assert!(fixture.read("package.json").contains("\"version\": \"1.2.4\""));
    assert!(fixture.read("Cargo.toml").contains("version = \"1.2.4\""));
    assert!(tag_exists(fixture, "v1.2.4"));
}

#[test]
fn bump_conventional_changelog_creates_features_and_fixes() {
    assert!(
        git_available(),
        "git CLI is required for e2e tests but was not found in PATH"
    );
    let guard = FixtureGuard::new("conv-cl-feat-fix");
    write_fixture(
        &guard,
        r#"[preflight]
check = "true""#,
    );
    let fixture = guard.fixture();
    run_git_ok(
        &fixture.dir,
        &["commit", "--allow-empty", "-m", "feat(parser): add AST builder"],
    );
    run_git_ok(
        &fixture.dir,
        &["commit", "--allow-empty", "-m", "fix: resolve memory leak on exit"],
    );

    let cfg = config::load("cutver.toml").unwrap();
    assert_eq!(cfg.changelog.mode, "conventional");
    let summary = bump_run(&cfg, BumpLevel::Auto, false, &[]).unwrap();
    assert_eq!(summary.next.to_string(), "1.3.0");
    assert!(!summary.dry_run);

    let cl = fixture.read("CHANGELOG.md");
    let today = cutver::changelog::format_date(SystemTime::now());
    assert!(cl.contains(&format!("## [v1.3.0] - {today}")));
    assert!(cl.contains("### Features\n- **parser**: add AST builder"));
    assert!(cl.contains("### Bug Fixes\n- resolve memory leak on exit"));
    assert!(!cl.contains("### ⚠️ Breaking Changes"));
    assert!(!cl.contains("Maintenance and updates."));
}

#[test]
fn bump_conventional_changelog_with_breaking_commit() {
    assert!(
        git_available(),
        "git CLI is required for e2e tests but was not found in PATH"
    );
    let guard = FixtureGuard::new("conv-cl-breaking");
    write_fixture(
        &guard,
        r#"[preflight]
check = "true""#,
    );
    let fixture = guard.fixture();
    run_git_ok(
        &fixture.dir,
        &[
            "commit",
            "--allow-empty",
            "-m",
            "feat(api)!: remove deprecated endpoints",
        ],
    );
    run_git_ok(
        &fixture.dir,
        &["commit", "--allow-empty", "-m", "fix: update error message"],
    );

    let cfg = config::load("cutver.toml").unwrap();
    let summary = bump_run(&cfg, BumpLevel::Auto, false, &[]).unwrap();
    assert_eq!(summary.next.to_string(), "2.0.0");
    assert!(!summary.dry_run);

    let cl = fixture.read("CHANGELOG.md");
    let today = cutver::changelog::format_date(SystemTime::now());
    assert!(cl.contains(&format!("## [v2.0.0] - {today}")));
    assert!(cl.contains("### ⚠️ Breaking Changes\n- **api**: remove deprecated endpoints"));
    assert!(cl.contains("### Bug Fixes\n- update error message"));
    assert!(!cl.contains("### Features"));
}

#[test]
fn bump_manual_level_with_conventional_changelog() {
    assert!(
        git_available(),
        "git CLI is required for e2e tests but was not found in PATH"
    );
    let guard = FixtureGuard::new("conv-cl-manual-minor");
    write_fixture(
        &guard,
        r#"[preflight]
check = "true""#,
    );
    let fixture = guard.fixture();
    run_git_ok(
        &fixture.dir,
        &["commit", "--allow-empty", "-m", "feat: manual minor feature"],
    );

    let cfg = config::load("cutver.toml").unwrap();
    let summary = bump_run(&cfg, Bump::Minor, false, &[]).unwrap();
    assert_eq!(summary.next.to_string(), "1.3.0");
    assert!(!summary.dry_run);

    let cl = fixture.read("CHANGELOG.md");
    assert!(cl.contains("### Features\n- manual minor feature"));
}

#[test]
fn bump_template_changelog_mode() {
    assert!(
        git_available(),
        "git CLI is required for e2e tests but was not found in PATH"
    );
    let guard = FixtureGuard::new("conv-cl-template");
    let fixture = guard.fixture();
    fixture.write("package.json", PACKAGE_JSON);
    fixture.write("Cargo.toml", CARGO_TOML);
    fixture.write("android/build.gradle.kts", GRADLE_KTS);
    fixture.write("CHANGELOG.md", CHANGELOG_MD);
    let toml = r#"[version]
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

[changelog]
path = "CHANGELOG.md"
mode = "template"
entry_template = "Static template notes."

[git]
tag_prefix = "v"
require_clean_tree = true
"#;
    fixture.write("cutver.toml", toml);
    init_git_repo(fixture);
    initial_commit(fixture);
    run_git_ok(&fixture.dir, &["commit", "--allow-empty", "-m", "feat: some feature"]);

    let cfg = config::load("cutver.toml").unwrap();
    let summary = bump_run(&cfg, BumpLevel::Auto, false, &[]).unwrap();
    assert_eq!(summary.next.to_string(), "1.3.0");
    let cl = fixture.read("CHANGELOG.md");
    assert!(cl.contains("Static template notes."));
    assert!(!cl.contains("### Features"));
}

#[test]
fn post_bump_runs_in_dry_run_and_real_bump() {
    assert!(git_available());
    let guard = FixtureGuard::new("post-bump-runs");
    let fixture = guard.fixture();
    let toml = r#"[version]
current_source = "package.json"

[[manifest]]
path = "package.json"
kind = "json"
field = "version"

[[manifest]]
path = "Cargo.toml"
kind = "cargo-package"

[changelog]
path = "CHANGELOG.md"

[git]
require_clean_tree = false

[hooks]
post_bump = "touch post_bump_ran.txt"
"#;
    fixture.write("package.json", PACKAGE_JSON);
    fixture.write("Cargo.toml", CARGO_TOML);
    fixture.write("CHANGELOG.md", CHANGELOG_MD);
    fixture.write("cutver.toml", toml);
    init_git_repo(fixture);
    initial_commit(fixture);

    let cfg = config::load("cutver.toml").unwrap();

    // Dry run: summary records post_bump, command NOT executed
    let dry_summary = bump_run(&cfg, Bump::Minor, true, &[]).unwrap();
    assert!(dry_summary.dry_run);
    assert_eq!(dry_summary.post_bump.as_deref(), Some("touch post_bump_ran.txt"));
    assert!(!fixture.dir.join("post_bump_ran.txt").exists());

    // Real run: post_bump executes and file is created
    let real_summary = bump_run(&cfg, Bump::Minor, false, &[]).unwrap();
    assert!(!real_summary.dry_run);
    assert_eq!(real_summary.post_bump.as_deref(), Some("touch post_bump_ran.txt"));
    assert!(fixture.dir.join("post_bump_ran.txt").exists());
}

#[test]
fn post_bump_modifies_file_and_file_is_staged_and_committed() {
    assert!(git_available());
    let guard = FixtureGuard::new("post-bump-modify");
    let fixture = guard.fixture();
    let toml = r#"[version]
current_source = "package.json"

[[manifest]]
path = "package.json"
kind = "json"
field = "version"

[[manifest]]
path = "Cargo.toml"
kind = "cargo-package"

[changelog]
path = "CHANGELOG.md"

[git]
require_clean_tree = true

[hooks]
post_bump = "echo \"lockfile-version-1.3.0\" > Cargo.lock"
"#;
    fixture.write("package.json", PACKAGE_JSON);
    fixture.write("Cargo.toml", CARGO_TOML);
    fixture.write("Cargo.lock", "lockfile-version-1.2.3\n");
    fixture.write("CHANGELOG.md", CHANGELOG_MD);
    fixture.write("cutver.toml", toml);
    init_git_repo(fixture);
    initial_commit(fixture);

    let cfg = config::load("cutver.toml").unwrap();
    let summary = bump_run(&cfg, Bump::Minor, false, &[]).unwrap();
    assert_eq!(summary.next.to_string(), "1.3.0");

    // Check that Cargo.lock was updated and included in the release commit
    assert_eq!(fixture.read("Cargo.lock").trim(), "lockfile-version-1.3.0");
    let commit_files = head_commit_files(fixture);
    assert!(
        commit_files.contains(&"Cargo.lock".to_string()),
        "commit_files: {commit_files:?}"
    );

    // Tree should be clean after bump
    let status_out = run_git(&fixture.dir, &["status", "--porcelain"]);
    assert!(String::from_utf8_lossy(&status_out.stdout).trim().is_empty());
}

#[test]
fn post_bump_stages_both_root_and_nested_lockfiles() {
    assert!(git_available());
    let guard = FixtureGuard::new("post-bump-root-and-nested-lockfiles");
    let fixture = guard.fixture();
    let toml = r#"[version]
current_source = "package.json"

[[manifest]]
path = "package.json"
kind = "json"
field = "version"

[[manifest]]
path = "Cargo.toml"
kind = "cargo-package"

[changelog]
path = "CHANGELOG.md"

[git]
require_clean_tree = true

[hooks]
post_bump = "echo \"lockfile-root-1.3.0\" > Cargo.lock && echo \"lockfile-nested-1.3.0\" > crates/core/Cargo.lock && echo \"lockfile-app-1.3.0\" > App/Cargo.lock"
"#;
    fixture.write("package.json", PACKAGE_JSON);
    fixture.write("Cargo.toml", CARGO_TOML);
    fixture.write("Cargo.lock", "lockfile-root-1.2.3\n");
    fixture.write("crates/core/Cargo.lock", "lockfile-nested-1.2.3\n");
    fixture.write("App/Cargo.lock", "lockfile-app-1.2.3\n");
    fixture.write("CHANGELOG.md", CHANGELOG_MD);
    fixture.write("cutver.toml", toml);
    init_git_repo(fixture);
    initial_commit(fixture);

    let cfg = config::load("cutver.toml").unwrap();
    let summary = bump_run(&cfg, Bump::Minor, false, &[]).unwrap();
    assert_eq!(summary.next.to_string(), "1.3.0");

    // All lockfiles were updated
    assert_eq!(fixture.read("Cargo.lock").trim(), "lockfile-root-1.3.0");
    assert_eq!(fixture.read("crates/core/Cargo.lock").trim(), "lockfile-nested-1.3.0");
    assert_eq!(fixture.read("App/Cargo.lock").trim(), "lockfile-app-1.3.0");

    // Both root and nested lockfiles must be included in the release commit
    let commit_files = head_commit_files(fixture);
    assert!(
        commit_files.contains(&"Cargo.lock".to_string()),
        "commit_files should contain root Cargo.lock: {commit_files:?}"
    );
    assert!(
        commit_files.contains(&"crates/core/Cargo.lock".to_string()),
        "commit_files should contain crates/core/Cargo.lock: {commit_files:?}"
    );
    assert!(
        commit_files.contains(&"App/Cargo.lock".to_string()),
        "commit_files should contain App/Cargo.lock: {commit_files:?}"
    );

    // Tree should be clean after bump
    let status_out = run_git(&fixture.dir, &["status", "--porcelain"]);
    assert!(
        String::from_utf8_lossy(&status_out.stdout).trim().is_empty(),
        "working tree must be clean after bump"
    );
}

#[test]
fn post_bump_restricts_staging_to_known_lockfiles_and_leaves_arbitrary_files_unstaged() {
    assert!(git_available());
    let guard = FixtureGuard::new("post-bump-lockfile-only");
    let fixture = guard.fixture();
    let toml = r#"[version]
current_source = "package.json"

[[manifest]]
path = "package.json"
kind = "json"
field = "version"

[[manifest]]
path = "Cargo.toml"
kind = "cargo-package"

[changelog]
path = "CHANGELOG.md"

[git]
require_clean_tree = true

[hooks]
post_bump = "echo \"lockfile-version-1.3.0\" > Cargo.lock && echo \"arbitrary-modification\" > unrelated.txt"
"#;
    fixture.write("package.json", PACKAGE_JSON);
    fixture.write("Cargo.toml", CARGO_TOML);
    fixture.write("Cargo.lock", "lockfile-version-1.2.3\n");
    fixture.write("unrelated.txt", "unrelated-initial\n");
    fixture.write("CHANGELOG.md", CHANGELOG_MD);
    fixture.write("cutver.toml", toml);
    init_git_repo(fixture);
    initial_commit(fixture);

    let cfg = config::load("cutver.toml").unwrap();
    let summary = bump_run(&cfg, Bump::Minor, false, &[]).unwrap();
    assert_eq!(summary.next.to_string(), "1.3.0");

    // Both files were modified by post_bump hook
    assert_eq!(fixture.read("Cargo.lock").trim(), "lockfile-version-1.3.0");
    assert_eq!(fixture.read("unrelated.txt").trim(), "arbitrary-modification");

    // ONLY Cargo.lock was staged and committed; unrelated.txt was not committed
    let commit_files = head_commit_files(fixture);
    assert!(
        commit_files.contains(&"Cargo.lock".to_string()),
        "commit_files should contain Cargo.lock: {commit_files:?}"
    );
    assert!(
        !commit_files.contains(&"unrelated.txt".to_string()),
        "commit_files should NOT contain unrelated.txt: {commit_files:?}"
    );

    // unrelated.txt remains unstaged in working tree
    let status_out = run_git(&fixture.dir, &["status", "--porcelain"]);
    let status_str = String::from_utf8_lossy(&status_out.stdout);
    assert!(
        status_str.contains("unrelated.txt"),
        "git status should report unrelated.txt: {status_str}"
    );
    assert!(
        status_str.lines().any(|l| l.starts_with(" M unrelated.txt")),
        "unrelated.txt must be unstaged (not in index): {status_str}"
    );

    // No files should remain staged in the index
    let diff_cached = run_git(&fixture.dir, &["diff", "--cached", "--name-only"]);
    assert!(
        String::from_utf8_lossy(&diff_cached.stdout).trim().is_empty(),
        "no files should remain staged in the index"
    );

    // Working tree diff should only show unrelated.txt
    let diff_unstaged = run_git(&fixture.dir, &["diff", "--name-only"]);
    let diff_unstaged_str = String::from_utf8_lossy(&diff_unstaged.stdout);
    let unstaged_files: Vec<&str> = diff_unstaged_str
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    assert_eq!(unstaged_files, vec!["unrelated.txt"]);
}

#[test]
fn post_bump_failure_triggers_rollback_and_aborts() {
    assert!(git_available());
    let guard = FixtureGuard::new("post-bump-fail");
    let fixture = guard.fixture();
    let toml = r#"[version]
current_source = "package.json"

[[manifest]]
path = "package.json"
kind = "json"
field = "version"

[[manifest]]
path = "Cargo.toml"
kind = "cargo-package"

[changelog]
path = "CHANGELOG.md"

[git]
require_clean_tree = true

[hooks]
post_bump = "exit 1"
"#;
    fixture.write("package.json", PACKAGE_JSON);
    fixture.write("Cargo.toml", CARGO_TOML);
    fixture.write("CHANGELOG.md", CHANGELOG_MD);
    fixture.write("cutver.toml", toml);
    init_git_repo(fixture);
    initial_commit(fixture);

    let cfg = config::load("cutver.toml").unwrap();
    let res = bump_run(&cfg, Bump::Minor, false, &[]);
    assert!(res.is_err(), "expected bump to fail due to post_bump exit 1");

    // Manifests rolled back to 1.2.3
    assert!(fixture.read("package.json").contains("\"version\": \"1.2.3\""));
    assert!(fixture.read("Cargo.toml").contains("version = \"1.2.3\""));
    assert!(!fixture.read("CHANGELOG.md").contains("1.3.0"));

    // No commit or tag created
    assert_eq!(commit_count(fixture), 1);
    assert!(!tag_exists(fixture, "v1.3.0"));
}

#[test]
fn publish_push_and_commands_reported_in_dry_run_and_executed_in_real_run() {
    assert!(git_available());
    let guard = FixtureGuard::new("publish-push-cmds");
    let fixture = guard.fixture();
    let remote = Fixture::new("remote-target");
    run_git_ok(&remote.dir, &["init", "--bare"]);

    let toml = r#"[version]
current_source = "package.json"

[[manifest]]
path = "package.json"
kind = "json"
field = "version"

[[manifest]]
path = "Cargo.toml"
kind = "cargo-package"

[changelog]
path = "CHANGELOG.md"

[git]
require_clean_tree = true
require_branch = "main"

[publish]
push = true
commands = ["echo published {version} > published.txt"]
"#;
    fixture.write("package.json", PACKAGE_JSON);
    fixture.write("Cargo.toml", CARGO_TOML);
    fixture.write("CHANGELOG.md", CHANGELOG_MD);
    fixture.write("cutver.toml", toml);
    init_git_repo(fixture);
    run_git_ok(&fixture.dir, &["checkout", "-B", "main"]);
    initial_commit(fixture);
    run_git_ok(&fixture.dir, &["remote", "add", "origin", remote.dir.to_str().unwrap()]);

    let cfg = config::load("cutver.toml").unwrap();

    // Dry run: reported in summary, neither push nor commands executed
    let dry_summary = bump_run(&cfg, Bump::Minor, true, &[]).unwrap();
    assert!(dry_summary.dry_run);
    assert!(dry_summary.publish_push);
    assert_eq!(
        dry_summary.publish_push_command.as_deref(),
        Some("git push origin main --tags")
    );
    assert_eq!(
        dry_summary.publish_commands,
        vec!["echo published 1.3.0 > published.txt"]
    );
    assert!(!fixture.dir.join("published.txt").exists());
    let remote_tags = run_git(&remote.dir, &["tag", "-l"]);
    assert!(String::from_utf8_lossy(&remote_tags.stdout).trim().is_empty());

    // Real run: push and commands executed
    let real_summary = bump_run(&cfg, Bump::Minor, false, &[]).unwrap();
    assert!(!real_summary.dry_run);
    assert!(real_summary.publish_push);
    assert_eq!(
        real_summary.publish_commands,
        vec!["echo published 1.3.0 > published.txt"]
    );
    assert!(fixture.dir.join("published.txt").exists());
    assert_eq!(fixture.read("published.txt").trim(), "published 1.3.0");

    // Verify tag pushed to remote
    let remote_tags = run_git(&remote.dir, &["tag", "-l"]);
    assert!(String::from_utf8_lossy(&remote_tags.stdout).contains("v1.3.0"));
}

#[test]
fn publish_push_resolves_active_branch_when_require_branch_is_omitted() {
    assert!(git_available());
    let guard = FixtureGuard::new("publish-push-no-require-branch");
    let fixture = guard.fixture();
    let remote = Fixture::new("remote-target-no-req-branch");
    run_git_ok(&remote.dir, &["init", "--bare"]);

    let toml = r#"[version]
current_source = "package.json"

[[manifest]]
path = "package.json"
kind = "json"
field = "version"

[[manifest]]
path = "Cargo.toml"
kind = "cargo-package"

[changelog]
path = "CHANGELOG.md"

[git]
require_clean_tree = true

[publish]
push = true
"#;
    fixture.write("package.json", PACKAGE_JSON);
    fixture.write("Cargo.toml", CARGO_TOML);
    fixture.write("CHANGELOG.md", CHANGELOG_MD);
    fixture.write("cutver.toml", toml);
    init_git_repo(fixture);
    run_git_ok(&fixture.dir, &["checkout", "-B", "release-branch"]);
    initial_commit(fixture);
    run_git_ok(&fixture.dir, &["remote", "add", "origin", remote.dir.to_str().unwrap()]);

    let cfg = config::load("cutver.toml").unwrap();

    // Dry run dynamically resolves active branch "release-branch"
    let dry_summary = bump_run(&cfg, Bump::Minor, true, &[]).unwrap();
    assert_eq!(
        dry_summary.publish_push_command.as_deref(),
        Some("git push origin release-branch --tags")
    );

    // Real run pushes release-branch and tag to remote
    let real_summary = bump_run(&cfg, Bump::Minor, false, &[]).unwrap();
    assert_eq!(real_summary.publish_push_command.as_deref(), None);

    // Verify tag pushed to remote
    let remote_tags = run_git(&remote.dir, &["tag", "-l"]);
    assert!(String::from_utf8_lossy(&remote_tags.stdout).contains("v1.3.0"));

    // Verify release-branch pushed to remote
    let remote_branches = run_git(&remote.dir, &["branch", "-l"]);
    assert!(String::from_utf8_lossy(&remote_branches.stdout).contains("release-branch"));
}

#[test]
fn publish_command_aborts_when_timing_out() {
    assert!(git_available());
    let guard = FixtureGuard::new("publish-cmd-timeout");
    let fixture = guard.fixture();

    let toml = r#"[version]
current_source = "package.json"

[[manifest]]
path = "package.json"
kind = "json"
field = "version"

[[manifest]]
path = "Cargo.toml"
kind = "cargo-package"

[changelog]
path = "CHANGELOG.md"

[git]
require_clean_tree = true
require_branch = "main"

[publish]
default_timeout = 1
commands = ["sleep 5"]
"#;
    fixture.write("package.json", PACKAGE_JSON);
    fixture.write("Cargo.toml", CARGO_TOML);
    fixture.write("CHANGELOG.md", CHANGELOG_MD);
    fixture.write("cutver.toml", toml);
    init_git_repo(fixture);
    run_git_ok(&fixture.dir, &["checkout", "-B", "main"]);
    initial_commit(fixture);

    let cfg = config::load("cutver.toml").unwrap();

    // Dry run remains unaffected (commands are recorded but not executed)
    let dry_summary = bump_run(&cfg, Bump::Minor, true, &[]).unwrap();
    assert_eq!(dry_summary.publish_commands, vec!["sleep 5"]);

    // Real run: command times out and aborts without hanging
    let start = std::time::Instant::now();
    let err = bump_run(&cfg, Bump::Minor, false, &[]).unwrap_err();
    let elapsed = start.elapsed();

    assert!(
        elapsed.as_secs() < 4,
        "command hung or did not abort promptly: took {:?}",
        elapsed
    );

    match err {
        cutver::bump::Error::PublishCommandTimeout {
            ref command,
            timeout,
            elapsed_ms,
        } => {
            assert_eq!(command, "sleep 5");
            assert_eq!(timeout, 1);
            assert!(elapsed_ms >= 1000);
        }
        other => panic!("expected PublishCommandTimeout, got: {:?}", other),
    }

    let err_str = err.to_string();
    assert!(
        err_str.contains("publish command 'sleep 5' timed out after"),
        "unexpected error message: {err_str}"
    );
    assert!(err_str.contains("(limit 1s)"), "unexpected error message: {err_str}");
}

#[test]
fn changelog_latest_cli_extracts_body_from_discovered_config() {
    let guard = FixtureGuard::new("changelog-latest-discovered");
    let fixture = guard.fixture();
    let cutver_toml = r#"[version]
current_source = "package.json"

[[manifest]]
path = "package.json"
kind = "json"
field = "version"

[changelog]
path = "CHANGELOG.md"
"#;
    fixture.write("cutver.toml", cutver_toml);
    fixture.write("package.json", r#"{"version": "1.2.0"}"#);
    let changelog_content = r#"# Changelog

## [Unreleased]
- work in progress

## [1.2.0] - 2026-03-01

### Features
- new thing
"#;
    fixture.write("CHANGELOG.md", changelog_content);

    let output = Command::new(env!("CARGO_BIN_EXE_cutver"))
        .current_dir(&fixture.dir)
        .args(["changelog", "latest"])
        .output()
        .expect("failed to execute cutver binary");

    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();
    assert!(
        trimmed.contains("### Features\n- new thing"),
        "stdout does not contain features: {stdout}"
    );
    assert!(
        !stdout.contains("## [1.2.0]"),
        "stdout should not contain release header: {stdout}"
    );
    assert!(
        !stdout.contains("[Unreleased]"),
        "stdout should not contain [Unreleased]: {stdout}"
    );
}

#[test]
fn changelog_latest_cli_with_include_header() {
    let guard = FixtureGuard::new("changelog-latest-header");
    let fixture = guard.fixture();
    let cutver_toml = r#"[version]
current_source = "package.json"

[[manifest]]
path = "package.json"
kind = "json"
field = "version"

[changelog]
path = "CHANGELOG.md"
"#;
    fixture.write("cutver.toml", cutver_toml);
    fixture.write("package.json", r#"{"version": "1.2.0"}"#);
    let changelog_content = r#"# Changelog

## [Unreleased]
- work in progress

## [1.2.0] - 2026-03-01

### Features
- new thing
"#;
    fixture.write("CHANGELOG.md", changelog_content);

    let output = Command::new(env!("CARGO_BIN_EXE_cutver"))
        .current_dir(&fixture.dir)
        .args(["changelog", "latest", "-H"])
        .output()
        .expect("failed to execute cutver binary");

    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.trim().starts_with("## [1.2.0] - 2026-03-01"),
        "stdout does not start with header: {stdout}"
    );
    assert!(
        stdout.contains("### Features\n- new thing"),
        "stdout does not contain features: {stdout}"
    );
}

#[test]
fn changelog_latest_cli_with_explicit_path() {
    let guard = FixtureGuard::new("changelog-latest-explicit-path");
    let fixture = guard.fixture();
    let releases_content = r#"# Releases

## [Unreleased]
- upcoming changes

## [2.0.0] - 2026-03-01

- custom release notes
"#;
    fixture.write("RELEASES.md", releases_content);

    let output = Command::new(env!("CARGO_BIN_EXE_cutver"))
        .current_dir(&fixture.dir)
        .args(["changelog", "latest", "-p", "RELEASES.md"])
        .output()
        .expect("failed to execute cutver binary");

    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "- custom release notes");
}

#[test]
fn changelog_latest_cli_fails_cleanly_when_no_releases() {
    let guard = FixtureGuard::new("changelog-latest-no-releases");
    let fixture = guard.fixture();
    let changelog_content = r#"# Changelog

## [Unreleased]
- work in progress
"#;
    fixture.write("CHANGELOG.md", changelog_content);

    let output = Command::new(env!("CARGO_BIN_EXE_cutver"))
        .current_dir(&fixture.dir)
        .args(["changelog", "latest"])
        .output()
        .expect("failed to execute cutver binary");

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("no release section found in changelog"),
        "stderr does not contain expected error: {stderr}"
    );
}

#[test]
fn changelog_show_cli_extracts_historical_version() {
    let guard = FixtureGuard::new("changelog-show-historical");
    let fixture = guard.fixture();
    let cutver_toml = r#"[version]
current_source = "package.json"

[[manifest]]
path = "package.json"
kind = "json"
field = "version"

[changelog]
path = "CHANGELOG.md"
"#;
    fixture.write("cutver.toml", cutver_toml);
    fixture.write("package.json", r#"{"version": "1.2.0"}"#);
    let changelog_content = r#"# Changelog

## [1.2.0] - 2026-03-01

- Release 1.2.0 notes

## [1.1.0] - 2026-02-01

- Release 1.1.0 notes

## [1.0.0] - 2026-01-01

- Release 1.0.0 notes
"#;
    fixture.write("CHANGELOG.md", changelog_content);

    let output = Command::new(env!("CARGO_BIN_EXE_cutver"))
        .current_dir(&fixture.dir)
        .args(["changelog", "show", "1.1.0"])
        .output()
        .expect("failed to execute cutver binary");

    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("- Release 1.1.0 notes"),
        "stdout does not contain 1.1.0 notes: {stdout}"
    );
    assert!(
        !stdout.contains("- Release 1.2.0 notes"),
        "stdout should not contain 1.2.0 notes: {stdout}"
    );
    assert!(
        !stdout.contains("- Release 1.0.0 notes"),
        "stdout should not contain 1.0.0 notes: {stdout}"
    );
    assert!(
        !stdout.contains("## [1.1.0]"),
        "stdout should not contain release header: {stdout}"
    );
}

#[test]
fn changelog_show_cli_with_include_header() {
    let guard = FixtureGuard::new("changelog-show-header");
    let fixture = guard.fixture();
    let cutver_toml = r#"[version]
current_source = "package.json"

[[manifest]]
path = "package.json"
kind = "json"
field = "version"

[changelog]
path = "CHANGELOG.md"
"#;
    fixture.write("cutver.toml", cutver_toml);
    fixture.write("package.json", r#"{"version": "1.2.0"}"#);
    let changelog_content = r#"# Changelog

## [1.2.0] - 2026-03-01

- Release 1.2.0 notes

## [1.1.0] - 2026-02-01

- Release 1.1.0 notes

## [1.0.0] - 2026-01-01

- Release 1.0.0 notes
"#;
    fixture.write("CHANGELOG.md", changelog_content);

    let output = Command::new(env!("CARGO_BIN_EXE_cutver"))
        .current_dir(&fixture.dir)
        .args(["changelog", "show", "1.1.0", "-H"])
        .output()
        .expect("failed to execute cutver binary");

    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.trim().starts_with("## [1.1.0]"),
        "stdout does not start with header ## [1.1.0]: {stdout}"
    );
    assert!(
        stdout.contains("- Release 1.1.0 notes"),
        "stdout does not contain 1.1.0 notes: {stdout}"
    );
}

#[test]
fn changelog_show_cli_with_version_prefix_v() {
    let guard = FixtureGuard::new("changelog-show-prefix-v");
    let fixture = guard.fixture();
    let cutver_toml = r#"[version]
current_source = "package.json"

[[manifest]]
path = "package.json"
kind = "json"
field = "version"

[changelog]
path = "CHANGELOG.md"
"#;
    fixture.write("cutver.toml", cutver_toml);
    fixture.write("package.json", r#"{"version": "1.2.0"}"#);
    let changelog_content = r#"# Changelog

## [1.2.0] - 2026-03-01

- Release 1.2.0 notes

## [1.1.0] - 2026-02-01

- Release 1.1.0 notes

## [1.0.0] - 2026-01-01

- Release 1.0.0 notes
"#;
    fixture.write("CHANGELOG.md", changelog_content);

    let output = Command::new(env!("CARGO_BIN_EXE_cutver"))
        .current_dir(&fixture.dir)
        .args(["changelog", "show", "v1.1.0"])
        .output()
        .expect("failed to execute cutver binary");

    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "- Release 1.1.0 notes");

    let output_header = Command::new(env!("CARGO_BIN_EXE_cutver"))
        .current_dir(&fixture.dir)
        .args(["changelog", "show", "v1.1.0", "-H"])
        .output()
        .expect("failed to execute cutver binary");

    assert!(
        output_header.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output_header.stderr)
    );
    assert_eq!(output_header.status.code(), Some(0));
    let stdout_header = String::from_utf8_lossy(&output_header.stdout);
    assert!(
        stdout_header.trim().starts_with("## [1.1.0]"),
        "stdout with -H does not match ## [1.1.0]: {stdout_header}"
    );
}

#[test]
fn changelog_show_cli_fails_cleanly_when_version_missing() {
    let guard = FixtureGuard::new("changelog-show-missing");
    let fixture = guard.fixture();
    let cutver_toml = r#"[version]
current_source = "package.json"

[[manifest]]
path = "package.json"
kind = "json"
field = "version"

[changelog]
path = "CHANGELOG.md"
"#;
    fixture.write("cutver.toml", cutver_toml);
    fixture.write("package.json", r#"{"version": "1.2.0"}"#);
    let changelog_content = r#"# Changelog

## [1.2.0] - 2026-03-01

- Release 1.2.0 notes
"#;
    fixture.write("CHANGELOG.md", changelog_content);

    let output = Command::new(env!("CARGO_BIN_EXE_cutver"))
        .current_dir(&fixture.dir)
        .args(["changelog", "show", "9.9.9"])
        .output()
        .expect("failed to execute cutver binary");

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("version '9.9.9' not found in changelog"),
        "stderr does not contain expected error: {stderr}"
    );
}

#[test]
fn doctor_with_check_changelog_cli_succeeds_when_consistent() {
    assert!(
        git_available(),
        "git CLI is required for e2e tests but was not found in PATH"
    );
    let guard = FixtureGuard::new("doctor-check-changelog-consistent");
    let fixture = guard.fixture();
    let cutver_toml = r#"[version]
current_source = "package.json"

[[manifest]]
path = "package.json"
kind = "json"
field = "version"
"#;
    fixture.write("cutver.toml", cutver_toml);
    fixture.write("package.json", r#"{"version": "1.1.0"}"#);
    init_git_repo(fixture);
    initial_commit(fixture);
    run_git_ok(&fixture.dir, &["tag", "v1.0.0"]);
    run_git_ok(&fixture.dir, &["tag", "v1.1.0"]);
    fixture.write(
        "CHANGELOG.md",
        "## [1.1.0] - 2026-02-01\n- v1.1.0\n\n## [1.0.0] - 2026-01-01\n- v1.0.0\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_cutver"))
        .current_dir(&fixture.dir)
        .args(["doctor", "--check-changelog"])
        .output()
        .expect("failed to execute cutver binary");

    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("cutver.toml is valid."),
        "stdout does not contain 'cutver.toml is valid.': {stdout}"
    );
    assert!(
        stdout.contains("changelog: consistent with Git tags"),
        "stdout does not contain 'changelog: consistent with Git tags': {stdout}"
    );
}

#[test]
fn doctor_with_check_changelog_cli_fails_with_drift() {
    assert!(
        git_available(),
        "git CLI is required for e2e tests but was not found in PATH"
    );
    let guard = FixtureGuard::new("doctor-check-changelog-drift");
    let fixture = guard.fixture();
    let cutver_toml = r#"[version]
current_source = "package.json"

[[manifest]]
path = "package.json"
kind = "json"
field = "version"
"#;
    fixture.write("cutver.toml", cutver_toml);
    fixture.write("package.json", r#"{"version": "1.1.0"}"#);
    init_git_repo(fixture);
    initial_commit(fixture);
    run_git_ok(&fixture.dir, &["tag", "v1.0.0"]);
    run_git_ok(&fixture.dir, &["tag", "v1.2.0"]);
    fixture.write(
        "CHANGELOG.md",
        "## [1.1.0] - 2026-02-01\n- v1.1.0\n\n## [1.0.0] - 2026-01-01\n- v1.0.0\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_cutver"))
        .current_dir(&fixture.dir)
        .args(["doctor", "--check-changelog"])
        .output()
        .expect("failed to execute cutver binary");

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Changelog drift detected:"),
        "stderr does not contain 'Changelog drift detected:': {stderr}"
    );
    assert!(
        stderr.contains("Missing in changelog"),
        "stderr does not contain 'Missing in changelog': {stderr}"
    );
    assert!(stderr.contains("v1.2.0"), "stderr does not contain 'v1.2.0': {stderr}");
    assert!(
        stderr.contains("Orphan changelog sections"),
        "stderr does not contain 'Orphan changelog sections': {stderr}"
    );
    assert!(stderr.contains("1.1.0"), "stderr does not contain '1.1.0': {stderr}");
}
