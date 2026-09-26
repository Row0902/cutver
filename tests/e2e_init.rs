#![cfg(unix)]

mod common;

use common::*;
use std::process::Command;

fn run_cutver(dir: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_cutver"))
        .current_dir(dir)
        .args(args)
        .output()
        .expect("failed to execute cutver binary")
}

#[test]
fn init_empty_dir_creates_starter_config() {
    let guard = FixtureGuard::new("init-empty");
    let fixture = guard.fixture();

    let output = run_cutver(&fixture.dir, &["init"]);
    assert!(
        output.status.success(),
        "cutver init failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("No package manifests were automatically discovered")
            || stdout.contains("starter cutver.toml")
            || stdout.contains("Initialized cutver"),
        "unexpected output: {stdout}"
    );

    assert!(fixture.dir.join("cutver.toml").is_file());
    assert!(fixture.dir.join("CHANGELOG.md").is_file());

    let changelog = fixture.read("CHANGELOG.md");
    assert!(changelog.contains("## [Unreleased]"));
}

#[test]
fn init_rust_project_creates_valid_config() {
    let guard = FixtureGuard::new("init-rust");
    let fixture = guard.fixture();

    fixture.write(
        "Cargo.toml",
        r#"[package]
name = "rust-demo"
version = "0.1.0"
edition = "2024"
"#,
    );

    let output = run_cutver(&fixture.dir, &["init"]);
    assert!(
        output.status.success(),
        "cutver init failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Cargo.toml"));
    assert!(stdout.contains("cargo-package"));
    assert!(stdout.contains("primary source of truth"));

    let config_content = fixture.read("cutver.toml");
    assert!(config_content.contains("commit_message = \"chore(release): v{version} [skip ci]\""));
    assert!(config_content.contains("require_branch = \"main\""));
    assert!(config_content.contains("push = true"));
    assert!(config_content.contains("kind = \"cargo-package\""));
    assert!(config_content.contains("path = \"Cargo.toml\""));
    assert!(config_content.contains("check = \"cargo check --workspace\""));
    assert!(config_content.contains("post_bump = \"cargo check --workspace\""));

    assert!(fixture.dir.join("CHANGELOG.md").is_file());

    // Validate that cutver doctor succeeds on the generated config
    let doctor_output = run_cutver(&fixture.dir, &["doctor"]);
    assert!(
        doctor_output.status.success(),
        "cutver doctor failed on generated config: {}",
        String::from_utf8_lossy(&doctor_output.stderr)
    );
}

#[test]
fn init_polyglot_project_orders_root_and_discovers_all() {
    let guard = FixtureGuard::new("init-polyglot");
    let fixture = guard.fixture();

    fixture.write(
        "package.json",
        r#"{
  "name": "poly-app",
  "version": "1.0.0"
}"#,
    );
    fixture.write(
        "Cargo.toml",
        r#"[package]
name = "poly-core"
version = "1.0.0"
edition = "2024"
"#,
    );
    fixture.write(
        "android/build.gradle.kts",
        r#"plugins { id("com.android.application") }
android {
    defaultConfig {
        versionName "1.0.0"
        versionCode 1
    }
}
"#,
    );

    let output = run_cutver(&fixture.dir, &["init"]);
    assert!(
        output.status.success(),
        "cutver init failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let config_content = fixture.read("cutver.toml");
    assert!(config_content.contains("path = \"package.json\""));
    assert!(config_content.contains("path = \"Cargo.toml\""));
    assert!(config_content.contains("path = \"android/build.gradle.kts\""));
    assert!(config_content.contains("kind = \"gradle\""));

    let doctor_output = run_cutver(&fixture.dir, &["doctor"]);
    assert!(
        doctor_output.status.success(),
        "cutver doctor failed on polyglot config: {}",
        String::from_utf8_lossy(&doctor_output.stderr)
    );
}

#[test]
fn init_fails_when_cutver_toml_exists_without_force() {
    let guard = FixtureGuard::new("init-collision");
    let fixture = guard.fixture();

    fixture.write("cutver.toml", "# custom existing\n");

    let output = run_cutver(&fixture.dir, &["init"]);
    assert!(
        !output.status.success(),
        "cutver init should fail when cutver.toml exists"
    );
    assert_eq!(output.status.code(), Some(1));

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("already exists"));
    assert!(stderr.contains("--update"));
    assert!(stderr.contains("--force"));
}

#[test]
fn init_force_overwrites_existing_config() {
    let guard = FixtureGuard::new("init-force");
    let fixture = guard.fixture();

    fixture.write("cutver.toml", "# obsolete config\n");
    fixture.write(
        "Cargo.toml",
        r#"[package]
name = "overwrite-demo"
version = "2.0.0"
edition = "2024"
"#,
    );

    let output = run_cutver(&fixture.dir, &["init", "--force"]);
    assert!(
        output.status.success(),
        "cutver init --force failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let config_content = fixture.read("cutver.toml");
    assert!(!config_content.contains("# obsolete config"));
    assert!(config_content.contains("path = \"Cargo.toml\""));
}

#[test]
fn init_update_appends_new_manifest_and_preserves_customizations() {
    let guard = FixtureGuard::new("init-update");
    let fixture = guard.fixture();

    let initial_toml = r#"# User custom comment
[version]
strategy = "conventional"

[[manifest]]
path = "Cargo.toml"
kind = "cargo-package"

[preflight]
test = "cargo test"

[git]
tag_prefix = "rel-"
"#;
    fixture.write("cutver.toml", initial_toml);
    fixture.write(
        "Cargo.toml",
        r#"[package]
name = "root-crate"
version = "0.1.0"
edition = "2024"
"#,
    );
    fixture.write(
        "crates/nested/Cargo.toml",
        r#"[package]
name = "nested-crate"
version = "0.1.0"
edition = "2024"
"#,
    );

    let output = run_cutver(&fixture.dir, &["init", "--update"]);
    assert!(
        output.status.success(),
        "cutver init --update failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Updated cutver.toml with 1 new manifest(s)"));
    assert!(stdout.contains("crates/nested/Cargo.toml"));

    let updated_toml = fixture.read("cutver.toml");
    assert!(updated_toml.contains("# User custom comment"));
    assert!(updated_toml.contains("test = \"cargo test\""));
    assert!(updated_toml.contains("tag_prefix = \"rel-\""));
    assert!(updated_toml.contains("path = \"crates/nested/Cargo.toml\""));

    // Second update should report already up to date
    let second_output = run_cutver(&fixture.dir, &["init", "-u"]);
    assert!(second_output.status.success());
    let second_stdout = String::from_utf8_lossy(&second_output.stdout);
    assert!(second_stdout.contains("already up to date"));
}
