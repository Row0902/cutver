#![cfg(unix)]

mod common;

use common::*;
use cutver::bump::run as bump_run;
use cutver::cli::BumpLevel;
use cutver::config;
use cutver::semver_bump::Bump;
use std::time::SystemTime;

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
