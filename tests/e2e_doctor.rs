#![cfg(unix)]

mod common;

use common::*;
use std::process::Command;

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
fn doctor_with_check_changelog_cli_succeeds_with_floating_major_tags() {
    assert!(
        git_available(),
        "git CLI is required for e2e tests but was not found in PATH"
    );
    let guard = FixtureGuard::new("doctor-check-changelog-floating-tags");
    let fixture = guard.fixture();
    let cutver_toml = r#"[version]
current_source = "package.json"

[[manifest]]
path = "package.json"
kind = "json"
field = "version"

[git]
floating_major_tag = true
"#;
    fixture.write("cutver.toml", cutver_toml);
    fixture.write("package.json", r#"{"version": "1.1.0"}"#);
    init_git_repo(fixture);
    initial_commit(fixture);
    run_git_ok(&fixture.dir, &["tag", "v1.0.0"]);
    run_git_ok(&fixture.dir, &["tag", "v1.1.0"]);
    run_git_ok(&fixture.dir, &["tag", "v1"]);
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
