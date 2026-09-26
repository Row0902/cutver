#![cfg(unix)]

mod common;

use common::*;
use cutver::bump::run as bump_run;
use cutver::config;
use cutver::semver_bump::Bump;

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
fn publish_push_with_floating_major_tag_pushes_floating_tag_to_remote() {
    assert!(git_available());
    let guard = FixtureGuard::new("publish-push-floating-tag");
    let fixture = guard.fixture();
    let remote = Fixture::new("remote-target-floating-tag");
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
tag_prefix = "v"
floating_major_tag = true
require_clean_tree = true
require_branch = "main"

[publish]
push = true
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

    // Dry run
    let dry_summary = bump_run(&cfg, Bump::Minor, true, &[]).unwrap();
    assert!(dry_summary.dry_run);
    assert!(dry_summary.publish_push);
    assert_eq!(
        dry_summary.publish_push_command.as_deref(),
        Some("git push origin +refs/tags/v1:refs/tags/v1 && git push origin main --tags")
    );

    // Real run: both v1.3.0 and v1 pushed to remote
    let real_summary = bump_run(&cfg, Bump::Minor, false, &[]).unwrap();
    assert!(!real_summary.dry_run);
    assert_eq!(real_summary.floating_tag.as_deref(), Some("v1"));

    let remote_tags = run_git(&remote.dir, &["tag", "-l"]);
    let remote_tags_str = String::from_utf8_lossy(&remote_tags.stdout);
    assert!(remote_tags_str.contains("v1.3.0"));
    assert!(remote_tags_str.contains("v1"));
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
fn post_bump_stages_modern_lockfiles() {
    assert!(
        git_available(),
        "git CLI is required for e2e tests but was not found in PATH"
    );
    let guard = FixtureGuard::new("post-bump-modern-lockfiles");
    let fixture = guard.fixture();
    let toml = r#"[version]
current_source = "package.json"

[[manifest]]
path = "package.json"
kind = "json"
field = "version"

[hooks]
post_bump = "echo bun-lock >> bun.lock && echo uv-lock >> uv.lock && echo pdm-lock >> pdm.lock"
"#;
    fixture.write("package.json", PACKAGE_JSON);
    fixture.write("cutver.toml", toml);
    fixture.write("bun.lock", "bun-lock-initial\n");
    fixture.write("uv.lock", "uv-lock-initial\n");
    fixture.write("pdm.lock", "pdm-lock-initial\n");
    init_git_repo(fixture);
    initial_commit(fixture);

    let cfg = config::load("cutver.toml").unwrap();
    let _summary = bump_run(&cfg, Bump::Patch, false, &[]).unwrap();

    let commit_files = head_commit_files(fixture);
    assert!(
        commit_files.contains(&"bun.lock".to_string()),
        "commit_files should contain bun.lock: {commit_files:?}"
    );
    assert!(
        commit_files.contains(&"uv.lock".to_string()),
        "commit_files should contain uv.lock: {commit_files:?}"
    );
    assert!(
        commit_files.contains(&"pdm.lock".to_string()),
        "commit_files should contain pdm.lock: {commit_files:?}"
    );
}
