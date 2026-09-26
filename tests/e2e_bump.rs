#![cfg(unix)]

mod common;

use common::*;
use cutver::bump::run as bump_run;
use cutver::config;
use cutver::semver_bump::Bump;
use std::process::Command;

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
fn bump_with_default_scaffolded_template() {
    assert!(
        git_available(),
        "git CLI is required for e2e tests but was not found in PATH"
    );
    let guard = FixtureGuard::new("bump-default-template");
    let fixture = guard.fixture();

    fixture.write(
        "Cargo.toml",
        r#"[package]
name = "test-pkg"
version = "0.1.0"
edition = "2024"
"#,
    );
    fixture.write("src/lib.rs", "// empty lib");

    init_git_repo(fixture);
    initial_commit(fixture);

    // Run cutver init to scaffold default config and template
    let init_out = Command::new(env!("CARGO_BIN_EXE_cutver"))
        .current_dir(&fixture.dir)
        .arg("init")
        .output()
        .expect("failed to execute cutver init");
    assert!(init_out.status.success());

    assert!(fixture.dir.join(".github/templates/cutver/RELEASE.md").is_file());

    // Disable publish push for local test
    let mut cfg_text = fixture.read("cutver.toml");
    cfg_text = cfg_text.replace("push = true", "push = false");
    fixture.write("cutver.toml", &cfg_text);

    run_git_ok(&fixture.dir, &["add", "."]);
    run_git_ok(&fixture.dir, &["commit", "-m", "chore: setup cutver"]);
    run_git_ok(&fixture.dir, &["tag", "v0.1.0"]);
    run_git_ok(
        &fixture.dir,
        &["commit", "--allow-empty", "-m", "feat: exciting feature (#10)"],
    );

    let bump_out = Command::new(env!("CARGO_BIN_EXE_cutver"))
        .current_dir(&fixture.dir)
        .args(["bump", "minor"])
        .output()
        .expect("failed to execute cutver bump");
    assert!(
        bump_out.status.success(),
        "bump failed: {}",
        String::from_utf8_lossy(&bump_out.stderr)
    );

    let cl = fixture.read("CHANGELOG.md");
    assert!(cl.contains("## [v0.2.0] - "));
    assert!(cl.contains("### Features"));
    assert!(cl.contains("- exciting feature (#10)"));
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
fn bump_pyproject_pep621_happy_path() {
    assert!(
        git_available(),
        "git CLI is required for e2e tests but was not found in PATH"
    );
    let guard = FixtureGuard::new("bump-pyproject-pep621");
    let fixture = guard.fixture();
    let pyproject = r#"[project]
name = "my-app"
version = "1.0.0" # current version
description = "Sample app"
"#;
    let cutver_toml = r#"[[manifest]]
path = "pyproject.toml"
kind = "pyproject"
"#;
    fixture.write("pyproject.toml", pyproject);
    fixture.write("cutver.toml", cutver_toml);
    init_git_repo(fixture);
    initial_commit(fixture);

    let cfg = config::load("cutver.toml").unwrap();
    let summary = bump_run(&cfg, Bump::Minor, false, &[]).unwrap();
    assert_eq!(summary.next.to_string(), "1.1.0");
    assert!(
        fixture
            .read("pyproject.toml")
            .contains("version = \"1.1.0\" # current version")
    );
    assert!(tag_exists(fixture, "v1.1.0"));
}

#[test]
fn bump_pyproject_poetry_happy_path() {
    assert!(
        git_available(),
        "git CLI is required for e2e tests but was not found in PATH"
    );
    let guard = FixtureGuard::new("bump-pyproject-poetry");
    let fixture = guard.fixture();
    let pyproject = r#"[tool.poetry]
name = "my-poetry-app"
version = "2.0.0" # poetry version
description = "Poetry app"
"#;
    let cutver_toml = r#"[[manifest]]
path = "pyproject.toml"
kind = "pyproject"
"#;
    fixture.write("pyproject.toml", pyproject);
    fixture.write("cutver.toml", cutver_toml);
    init_git_repo(fixture);
    initial_commit(fixture);

    let cfg = config::load("cutver.toml").unwrap();
    let summary = bump_run(&cfg, Bump::Patch, false, &[]).unwrap();
    assert_eq!(summary.next.to_string(), "2.0.1");
    assert!(
        fixture
            .read("pyproject.toml")
            .contains("version = \"2.0.1\" # poetry version")
    );
}

#[test]
fn bump_first_release_preserves_version_and_tags_initial_version() {
    assert!(
        git_available(),
        "git CLI is required for e2e tests but was not found in PATH"
    );
    let guard = FixtureGuard::new("bump-first-release");
    let fixture = guard.fixture();
    let package_json = r#"{
  "name": "my-initial-pkg",
  "version": "0.1.0"
}
"#;
    let cargo_toml = r#"[package]
name = "my-initial-pkg"
version = "0.1.0"
edition = "2021"
"#;
    let changelog_md = r#"# Changelog
All notable changes to this project will be documented in this file.

"#;
    let cutver_toml = r#"[[manifest]]
path = "package.json"
kind = "json"
field = "version"

[[manifest]]
path = "Cargo.toml"
kind = "cargo-package"

[changelog]
path = "CHANGELOG.md"
mode = "conventional"

[git]
tag_prefix = "v"
require_clean_tree = true
"#;
    fixture.write("package.json", package_json);
    fixture.write("Cargo.toml", cargo_toml);
    fixture.write("CHANGELOG.md", changelog_md);
    fixture.write("cutver.toml", cutver_toml);
    init_git_repo(fixture);
    initial_commit(fixture);
    run_git_ok(
        &fixture.dir,
        &["commit", "--allow-empty", "-m", "feat: initial feature commit"],
    );

    let cfg = config::load("cutver.toml").unwrap();
    let summary = cutver::bump::run_with_first_release(&cfg, cutver::cli::BumpLevel::Auto, false, &[], true).unwrap();

    // Resulting version is still 0.1.0
    assert_eq!(summary.current.to_string(), "0.1.0");
    assert_eq!(summary.next.to_string(), "0.1.0");
    assert!(fixture.read("package.json").contains("\"version\": \"0.1.0\""));
    assert!(fixture.read("Cargo.toml").contains("version = \"0.1.0\""));

    // Tag v0.1.0 is created
    assert!(tag_exists(fixture, "v0.1.0"));
    assert!(is_annotated_tag(fixture, "v0.1.0"));

    // CHANGELOG.md has ## [0.1.0] with initial commits
    let cl = fixture.read("CHANGELOG.md");
    assert!(cl.contains("0.1.0"));
    assert!(cl.contains("### Features\n- initial feature commit"));
}

#[test]
fn bump_with_floating_major_tag_creates_and_updates_tag() {
    assert!(
        git_available(),
        "git CLI is required for e2e tests but was not found in PATH"
    );
    let guard = FixtureGuard::new("bump-floating-major-tag");
    let fixture = guard.fixture();
    let package_json = r#"{
  "name": "my-floating-pkg",
  "version": "1.0.0"
}
"#;
    let changelog_md = r#"# Changelog
All notable changes to this project will be documented in this file.

## [1.0.0] - 2026-01-01
- initial release
"#;
    let cutver_toml = r#"[[manifest]]
path = "package.json"
kind = "json"
field = "version"

[changelog]
path = "CHANGELOG.md"
mode = "conventional"

[git]
tag_prefix = "v"
floating_major_tag = true
require_clean_tree = true
"#;
    fixture.write("package.json", package_json);
    fixture.write("CHANGELOG.md", changelog_md);
    fixture.write("cutver.toml", cutver_toml);
    init_git_repo(fixture);
    initial_commit(fixture);
    run_git_ok(&fixture.dir, &["tag", "v1.0.0"]);
    run_git_ok(&fixture.dir, &["tag", "v1"]);

    // Make a commit to trigger minor bump
    fixture.write("feature.txt", "new feature");
    run_git_ok(&fixture.dir, &["add", "."]);
    run_git_ok(&fixture.dir, &["commit", "-m", "feat: exciting new feature"]);

    let cfg = config::load("cutver.toml").unwrap();
    let summary = cutver::bump::run(&cfg, cutver::cli::BumpLevel::Auto, false, &[]).unwrap();

    assert_eq!(summary.next.to_string(), "1.1.0");
    assert_eq!(summary.floating_tag, Some("v1".to_string()));

    // Verify both tags exist
    assert!(tag_exists(fixture, "v1.1.0"));
    assert!(tag_exists(fixture, "v1"));

    // Verify v1 points to the exact same commit as v1.1.0
    let v1_commit = cutver::git::rev_parse(&fixture.dir, "v1^{commit}").unwrap();
    let v1_1_0_commit = cutver::git::rev_parse(&fixture.dir, "v1.1.0^{commit}").unwrap();
    assert_eq!(v1_commit, v1_1_0_commit);

    // Verify latest_tag resolves v1.1.0, ignoring floating tag v1
    let latest = cutver::git::latest_tag(&fixture.dir, Some("v")).unwrap();
    assert_eq!(latest, Some("v1.1.0".to_string()));
}
