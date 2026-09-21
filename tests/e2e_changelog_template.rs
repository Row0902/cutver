#![cfg(unix)]

mod common;

use common::*;
use std::process::Command;

#[test]
fn test_inline_changelog_template_in_cutver_toml() {
    assert!(
        git_available(),
        "git CLI is required for e2e tests but was not found in PATH"
    );
    let guard = FixtureGuard::new("cl-inline-template");
    let fixture = guard.fixture();

    let cutver_toml = r#"[version]
current_source = "package.json"

[[manifest]]
path = "package.json"
kind = "json"
field = "version"

[changelog]
path = "CHANGELOG.md"
template = """
## Release {{ version }} ({{ date }})
{% if features %}
### Features
{{ features }}
{% endif %}
{% if fixes %}
### Fixes
{{ fixes }}
{% endif %}
"""

[git]
tag_prefix = "v"
require_clean_tree = true
"#;
    fixture.write("cutver.toml", cutver_toml);
    fixture.write("package.json", r#"{"version": "1.0.0"}"#);
    fixture.write(
        "CHANGELOG.md",
        "# Changelog\n\n## [1.0.0] - 2026-01-01\n\n- Initial release\n",
    );

    init_git_repo(fixture);
    initial_commit(fixture);
    run_git_ok(
        &fixture.dir,
        &["commit", "--allow-empty", "-m", "feat(api): add v2 endpoint"],
    );

    let output = Command::new(env!("CARGO_BIN_EXE_cutver"))
        .current_dir(&fixture.dir)
        .args(["bump", "minor"])
        .output()
        .expect("failed to execute cutver bump");

    assert!(
        output.status.success(),
        "bump command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let cl = fixture.read("CHANGELOG.md");
    assert!(
        cl.contains("## Release 1.1.0"),
        "CHANGELOG.md should contain rendered release heading: {cl}"
    );
    assert!(
        cl.contains("### Features"),
        "CHANGELOG.md should contain Features section: {cl}"
    );
    assert!(
        cl.contains("- **api**: add v2 endpoint"),
        "CHANGELOG.md should contain conventional commit item: {cl}"
    );
}

#[test]
fn test_external_template_file_custom_filename() {
    assert!(
        git_available(),
        "git CLI is required for e2e tests but was not found in PATH"
    );
    let guard = FixtureGuard::new("cl-template-file");
    let fixture = guard.fixture();

    let cutver_toml = r#"[version]
current_source = "package.json"

[[manifest]]
path = "package.json"
kind = "json"
field = "version"

[changelog]
path = "CHANGELOG.md"
template_file = "templates/custom_notes_format.j2"

[git]
tag_prefix = "v"
require_clean_tree = true
"#;
    fixture.write("cutver.toml", cutver_toml);
    fixture.write("package.json", r#"{"version": "2.0.0"}"#);
    fixture.write(
        "CHANGELOG.md",
        "# Changelog\n\n## [2.0.0] - 2026-01-01\n\n- Base version\n",
    );

    let template_content = r#"🚀 Release {{ tag }} ({{ version }}) - {{ date }}

{% if features %}
New Capabilities:
{{ features }}
{% endif %}

Contributors:
{% for author in contributors -%}
- @{{ author }}
{% endfor %}
"#;
    fixture.write("templates/custom_notes_format.j2", template_content);

    init_git_repo(fixture);
    initial_commit(fixture);
    run_git_ok(
        &fixture.dir,
        &["commit", "--allow-empty", "-m", "feat: brand new feature"],
    );

    let output = Command::new(env!("CARGO_BIN_EXE_cutver"))
        .current_dir(&fixture.dir)
        .args(["bump", "minor"])
        .output()
        .expect("failed to execute cutver bump");

    assert!(
        output.status.success(),
        "bump command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let cl = fixture.read("CHANGELOG.md");
    assert!(
        cl.contains("🚀 Release v2.1.0 (2.1.0)"),
        "CHANGELOG.md should contain custom header: {cl}"
    );
    assert!(
        cl.contains("New Capabilities:"),
        "CHANGELOG.md should contain custom section: {cl}"
    );
    assert!(
        cl.contains("- brand new feature"),
        "CHANGELOG.md should contain feature item: {cl}"
    );
    assert!(
        cl.contains("Contributors:"),
        "CHANGELOG.md should contain contributors section: {cl}"
    );
}

#[test]
fn test_cli_template_flag_formatting_latest_and_show() {
    let guard = FixtureGuard::new("cl-cli-template-flag");
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

### Features
- awesome feature
- another cool feature

### Bug Fixes
- fixed crash

## [1.1.0] - 2026-02-01

### Features
- old feature
"#;
    fixture.write("CHANGELOG.md", changelog_content);

    let custom_template = "Release: {{ version }}\n{{ all_changes }}";
    fixture.write("notes.j2", custom_template);

    let output_latest = Command::new(env!("CARGO_BIN_EXE_cutver"))
        .current_dir(&fixture.dir)
        .args(["changelog", "latest", "--template", "notes.j2"])
        .output()
        .expect("failed to execute cutver changelog latest");

    assert!(
        output_latest.status.success(),
        "changelog latest failed: {}",
        String::from_utf8_lossy(&output_latest.stderr)
    );
    let stdout_latest = String::from_utf8_lossy(&output_latest.stdout);
    assert!(
        stdout_latest.contains("Release: 1.2.0"),
        "stdout should format latest version: {stdout_latest}"
    );
    assert!(
        stdout_latest.contains("awesome feature"),
        "stdout should contain changes: {stdout_latest}"
    );

    let output_show = Command::new(env!("CARGO_BIN_EXE_cutver"))
        .current_dir(&fixture.dir)
        .args(["changelog", "show", "1.1.0", "--template", "notes.j2"])
        .output()
        .expect("failed to execute cutver changelog show");

    assert!(
        output_show.status.success(),
        "changelog show failed: {}",
        String::from_utf8_lossy(&output_show.stderr)
    );
    let stdout_show = String::from_utf8_lossy(&output_show.stdout);
    assert!(
        stdout_show.contains("Release: 1.1.0"),
        "stdout should format requested version: {stdout_show}"
    );
    assert!(
        stdout_show.contains("old feature"),
        "stdout should contain historical changes: {stdout_show}"
    );
}

#[test]
fn test_backward_compatibility_no_template_specified() {
    assert!(
        git_available(),
        "git CLI is required for e2e tests but was not found in PATH"
    );
    let guard = FixtureGuard::new("cl-backward-compat");
    let fixture = guard.fixture();

    let cutver_toml = r#"[version]
current_source = "package.json"

[[manifest]]
path = "package.json"
kind = "json"
field = "version"

[changelog]
path = "CHANGELOG.md"

[git]
tag_prefix = "v"
require_clean_tree = true
"#;
    fixture.write("cutver.toml", cutver_toml);
    fixture.write("package.json", r#"{"version": "1.0.0"}"#);
    fixture.write(
        "CHANGELOG.md",
        "# Changelog\n\n## [1.0.0] - 2026-01-01\n\n- Initial release\n",
    );

    init_git_repo(fixture);
    initial_commit(fixture);
    run_git_ok(
        &fixture.dir,
        &["commit", "--allow-empty", "-m", "fix: fix edge case bug"],
    );

    let output = Command::new(env!("CARGO_BIN_EXE_cutver"))
        .current_dir(&fixture.dir)
        .args(["bump", "patch"])
        .output()
        .expect("failed to execute cutver bump");

    assert!(
        output.status.success(),
        "bump command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let cl = fixture.read("CHANGELOG.md");
    assert!(
        cl.contains("## [v1.0.1] - "),
        "CHANGELOG.md should contain standard keep-a-changelog header: {cl}"
    );
    assert!(
        cl.contains("### Bug Fixes"),
        "CHANGELOG.md should contain standard Bug Fixes header: {cl}"
    );
    assert!(
        cl.contains("- fix edge case bug"),
        "CHANGELOG.md should contain fix item: {cl}"
    );

    let output_latest = Command::new(env!("CARGO_BIN_EXE_cutver"))
        .current_dir(&fixture.dir)
        .args(["changelog", "latest"])
        .output()
        .expect("failed to execute cutver changelog latest");

    assert!(output_latest.status.success());
    let stdout = String::from_utf8_lossy(&output_latest.stdout);
    assert!(stdout.contains("### Bug Fixes"));
    assert!(stdout.contains("- fix edge case bug"));
}
