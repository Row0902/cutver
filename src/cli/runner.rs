use std::path::{Path, PathBuf};

use crate::bump::{self, Drift, Summary};
use crate::cli::args::{BumpLevel, ChangelogCommands, Cli, Commands};
use crate::config;

pub fn run(args: Cli) -> i32 {
    match args.command {
        Commands::Changelog { command } => run_changelog(args.config.as_deref(), command),
        Commands::Doctor { check_changelog } => {
            let config = match load_config(args.config) {
                Ok(c) => c,
                Err(code) => return code,
            };
            run_doctor(&config, check_changelog)
        }
        Commands::Bump {
            level,
            dry_run,
            skip_preflight,
        } => {
            let config = match load_config(args.config) {
                Ok(c) => c,
                Err(code) => return code,
            };
            run_bump(&config, level, dry_run, &skip_preflight)
        }
    }
}

pub fn load_config(config_path: Option<PathBuf>) -> Result<config::Config, i32> {
    let config = match config_path {
        Some(path) => config::load(path),
        None => {
            let start_dir = match std::env::current_dir() {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("Error: unable to determine current directory: {e}");
                    return Err(1);
                }
            };
            config::discover(start_dir)
        }
    };
    config.map_err(|e| {
        eprintln!("Error loading config: {e}");
        1
    })
}

pub fn resolve_changelog_path(config_override: Option<&Path>, path: Option<PathBuf>) -> Result<PathBuf, i32> {
    if let Some(p) = path {
        return Ok(p);
    }

    let config_result = match config_override {
        Some(p) => config::load(p),
        None => match std::env::current_dir() {
            Ok(dir) => config::discover(dir),
            Err(e) => {
                eprintln!("Error: unable to determine current directory: {e}");
                return Err(1);
            }
        },
    };

    match config_result {
        Ok(config) => {
            if let Some(ref cl_path) = config.changelog.path {
                let cl_pb = Path::new(cl_path);
                if cl_pb.is_absolute() {
                    Ok(cl_pb.to_path_buf())
                } else {
                    Ok(config.root_dir.join(cl_pb))
                }
            } else {
                let candidate = config.root_dir.join("CHANGELOG.md");
                if candidate.exists() {
                    Ok(candidate)
                } else {
                    let fallback = PathBuf::from("CHANGELOG.md");
                    if fallback.exists() {
                        Ok(fallback)
                    } else {
                        eprintln!("Error: no changelog path configured and CHANGELOG.md not found");
                        Err(1)
                    }
                }
            }
        }
        Err(e) => {
            let fallback = PathBuf::from("CHANGELOG.md");
            if fallback.exists() {
                Ok(fallback)
            } else {
                eprintln!("Error: {e}");
                Err(1)
            }
        }
    }
}

pub fn run_changelog(config_override: Option<&Path>, command: ChangelogCommands) -> i32 {
    match command {
        ChangelogCommands::Latest { include_header, path } => {
            let target_path = match resolve_changelog_path(config_override, path) {
                Ok(p) => p,
                Err(code) => return code,
            };

            match crate::changelog::read_latest(&target_path, include_header) {
                Ok(output) => {
                    println!("{output}");
                    0
                }
                Err(e) => {
                    eprintln!("Error: {e}");
                    1
                }
            }
        }
        ChangelogCommands::Show {
            version,
            include_header,
            path,
        } => {
            let target_path = match resolve_changelog_path(config_override, path) {
                Ok(p) => p,
                Err(code) => return code,
            };

            match crate::changelog::read_version(&target_path, &version, include_header) {
                Ok(output) => {
                    println!("{output}");
                    0
                }
                Err(e) => {
                    eprintln!("Error: {e}");
                    1
                }
            }
        }
    }
}

pub fn run_bump(config: &config::Config, level: BumpLevel, dry_run: bool, skip_preflight: &[String]) -> i32 {
    match bump::run(config, level, dry_run, skip_preflight) {
        Ok(summary) => {
            print_summary(&summary);
            0
        }
        Err(e) => {
            eprintln!("Error: {e}");
            1
        }
    }
}

pub fn run_doctor(config: &config::Config, check_changelog: bool) -> i32 {
    let mut has_drift = false;

    match bump::doctor(config) {
        Ok(drifts) => {
            if !drifts.is_empty() {
                eprintln!("Drift detected ({} manifest(s) out of sync):", drifts.len());
                for Drift { path, expected, actual } in drifts {
                    eprintln!("  - {path}: expected {expected}, found {actual}");
                }
                has_drift = true;
            }
        }
        Err(e) => {
            eprintln!("Error: {e}");
            return 1;
        }
    }

    if check_changelog {
        match bump::doctor_changelog(config) {
            Ok(cl_drift) => {
                if !cl_drift.is_empty() {
                    eprintln!("Changelog drift detected:");
                    if !cl_drift.missing_in_changelog.is_empty() {
                        eprintln!("  Missing in changelog (Git tag exists):");
                        for tag in &cl_drift.missing_in_changelog {
                            eprintln!("    - {tag}");
                        }
                    }
                    if !cl_drift.orphan_sections.is_empty() {
                        eprintln!("  Orphan changelog sections (no Git tag exists):");
                        for sec in &cl_drift.orphan_sections {
                            eprintln!("    - {sec}");
                        }
                    }
                    has_drift = true;
                }
            }
            Err(e) => {
                eprintln!("Error checking changelog: {e}");
                return 1;
            }
        }
    }

    if has_drift {
        return 2;
    }

    let filename = if config.root_dir.join("release.toml").is_file() && !config.root_dir.join("cutver.toml").is_file() {
        "release.toml"
    } else {
        "cutver.toml"
    };
    println!("{filename} is valid.");
    println!("  manifests: {}", config.manifest.len());
    println!("  preflight steps: {}", config.preflight.len());
    println!("  current source: {}", config.version.current_source);
    if check_changelog {
        println!("  changelog: consistent with Git tags");
    }
    0
}

pub fn print_summary(summary: &Summary) {
    println!("Bump summary:");
    println!("  source: {}", summary.source);
    println!("  current version: {}", summary.current);
    println!("  next version: {}", summary.next);
    println!("  dry run: {}", summary.dry_run);
    println!("  preflight commands:");
    for step in &summary.preflight {
        let marker = if step.skipped { " [SKIPPED]" } else { "" };
        println!("    - {}: {}{}", step.name, step.command, marker);
    }
    println!("  manifests:");
    for t in &summary.touched {
        println!("    - {}: {} -> {}", t.path, t.old, t.new);
    }
    if let Some(cl) = &summary.changelog {
        println!("  changelog: {cl}");
    }
    if let Some(hook) = &summary.post_bump {
        println!("  post_bump: {hook}");
    }
    println!("  commit: {}", summary.commit_message);
    println!("  tag: {}", summary.tag);
    if summary.publish_push || !summary.publish_commands.is_empty() {
        println!("  publish:");
        if let Some(cmd) = &summary.publish_push_command {
            println!("    push: {cmd}");
        } else if summary.publish_push {
            println!("    push: true");
        }
        if !summary.publish_commands.is_empty() {
            println!("    commands:");
            for cmd in &summary.publish_commands {
                println!("      - {cmd}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    static TMP_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    fn tmp_id(prefix: &str) -> String {
        let n = TMP_SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        format!("{}-{}-{}", prefix, std::process::id(), n)
    }

    use super::*;
    use clap::Parser;
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;

    fn temp_dir(prefix: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(tmp_id(prefix));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(dir: &Path, name: &str, text: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, text).unwrap();
        path
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
    fn doctor_reports_valid_config() {
        let dir = temp_dir("cutver-doc-ok");
        write(&dir, "package.json", r#"{"version": "1.0.0"}"#);
        write(
            &dir,
            "release.toml",
            r#"
[version]
current_source = "package.json"
[[manifest]]
path = "package.json"
kind = "json"
field = "version"
"#,
        );
        let args =
            Cli::try_parse_from(["cutver", "-c", &dir.join("release.toml").to_string_lossy(), "doctor"]).unwrap();
        assert_eq!(run(args), 0);
    }

    #[test]
    fn doctor_fails_for_invalid_config() {
        let dir = temp_dir("cutver-doc-bad");
        write(&dir, "release.toml", "[version]\ncurrent_source = \"missing\"");
        let args =
            Cli::try_parse_from(["cutver", "-c", &dir.join("release.toml").to_string_lossy(), "doctor"]).unwrap();
        assert_eq!(run(args), 1);
    }

    #[test]
    fn doctor_reports_drift_exit_code() {
        let dir = temp_dir("cutver-doc-drift-exit");
        write(&dir, "package.json", r#"{"version": "1.2.3"}"#);
        write(&dir, "Cargo.toml", "[package]\nversion = \"1.0.0\"\n");
        write(
            &dir,
            "release.toml",
            r#"
[version]
current_source = "package.json"
[[manifest]]
path = "package.json"
kind = "json"
field = "version"
[[manifest]]
path = "Cargo.toml"
kind = "cargo-package"
"#,
        );
        let args =
            Cli::try_parse_from(["cutver", "-c", &dir.join("release.toml").to_string_lossy(), "doctor"]).unwrap();
        assert_eq!(run(args), 2);
    }

    #[test]
    fn doctor_with_check_changelog_consistent() {
        let dir = temp_dir("cutver-doc-cl-main-ok");
        crate::git::init_test_repo(&dir);
        write(&dir, "package.json", r#"{"version": "1.0.0"}"#);
        write(
            &dir,
            "CHANGELOG.md",
            "# Changelog\n\n## [1.0.0] - 2026-01-01\n- initial\n",
        );
        write(
            &dir,
            "release.toml",
            r#"
[version]
current_source = "package.json"
[[manifest]]
path = "package.json"
kind = "json"
field = "version"
"#,
        );
        git_commit(&dir, "chore: initial");
        git_tag(&dir, "v1.0.0");

        let args = Cli::try_parse_from([
            "cutver",
            "-c",
            &dir.join("release.toml").to_string_lossy(),
            "doctor",
            "--check-changelog",
        ])
        .unwrap();
        assert_eq!(run(args), 0);
    }

    #[test]
    fn doctor_with_check_changelog_drift_missing_in_changelog() {
        let dir = temp_dir("cutver-doc-cl-main-missing");
        crate::git::init_test_repo(&dir);
        write(&dir, "package.json", r#"{"version": "1.1.0"}"#);
        write(
            &dir,
            "CHANGELOG.md",
            "# Changelog\n\n## [1.0.0] - 2026-01-01\n- initial\n",
        );
        write(
            &dir,
            "release.toml",
            r#"
[version]
current_source = "package.json"
[[manifest]]
path = "package.json"
kind = "json"
field = "version"
"#,
        );
        git_commit(&dir, "chore: initial");
        git_tag(&dir, "v1.0.0");
        git_tag(&dir, "v1.1.0");

        let args = Cli::try_parse_from([
            "cutver",
            "-c",
            &dir.join("release.toml").to_string_lossy(),
            "doctor",
            "--check-changelog",
        ])
        .unwrap();
        assert_eq!(run(args), 2);
    }

    #[test]
    fn doctor_with_check_changelog_orphan_section() {
        let dir = temp_dir("cutver-doc-cl-main-orphan");
        crate::git::init_test_repo(&dir);
        write(&dir, "package.json", r#"{"version": "1.0.0"}"#);
        write(
            &dir,
            "CHANGELOG.md",
            "# Changelog\n\n## [1.2.0] - 2026-01-02\n- unreleased\n\n## [1.0.0] - 2026-01-01\n- initial\n",
        );
        write(
            &dir,
            "release.toml",
            r#"
[version]
current_source = "package.json"
[[manifest]]
path = "package.json"
kind = "json"
field = "version"
"#,
        );
        git_commit(&dir, "chore: initial");
        git_tag(&dir, "v1.0.0");

        let args = Cli::try_parse_from([
            "cutver",
            "-c",
            &dir.join("release.toml").to_string_lossy(),
            "doctor",
            "--check-changelog",
        ])
        .unwrap();
        assert_eq!(run(args), 2);
    }

    #[test]
    fn doctor_with_check_changelog_error() {
        let dir = temp_dir("cutver-doc-cl-main-err");
        crate::git::init_test_repo(&dir);
        write(&dir, "package.json", r#"{"version": "1.0.0"}"#);
        write(
            &dir,
            "release.toml",
            r#"
[version]
current_source = "package.json"
[changelog]
path = "NONEXISTENT.md"
[[manifest]]
path = "package.json"
kind = "json"
field = "version"
"#,
        );
        let args = Cli::try_parse_from([
            "cutver",
            "-c",
            &dir.join("release.toml").to_string_lossy(),
            "doctor",
            "--check-changelog",
        ])
        .unwrap();
        assert_eq!(run(args), 1);
    }

    #[test]
    fn bump_dry_run_does_not_mutate() {
        let dir = temp_dir("cutver-bump-stub");
        crate::git::init_test_repo(&dir);
        write(&dir, "package.json", r#"{"version": "1.2.3"}"#);
        write(&dir, "Cargo.toml", "[package]\nversion = \"1.2.3\"\n");
        write(
            &dir,
            "release.toml",
            r#"
[version]
current_source = "package.json"
[[manifest]]
path = "package.json"
kind = "json"
field = "version"
[[manifest]]
path = "Cargo.toml"
kind = "cargo-package"
[git]
require_clean_tree = false
[preflight]
tests = "cargo test"
"#,
        );
        let cargo_before = fs::read_to_string(dir.join("Cargo.toml")).unwrap();
        let args = Cli::try_parse_from([
            "cutver",
            "-c",
            &dir.join("release.toml").to_string_lossy(),
            "bump",
            "minor",
            "--dry-run",
            "--skip-preflight",
            "tests",
        ])
        .unwrap();
        assert_eq!(run(args), 0);
        assert_eq!(fs::read_to_string(dir.join("Cargo.toml")).unwrap(), cargo_before);
    }

    #[test]
    fn bump_auto_dry_run_with_feat() {
        let dir = temp_dir("cutver-bump-auto-main");
        crate::git::init_test_repo(&dir);
        write(&dir, "package.json", r#"{"version": "1.2.3"}"#);
        write(&dir, "Cargo.toml", "[package]\nversion = \"1.2.3\"\n");
        write(
            &dir,
            "release.toml",
            r#"
[version]
current_source = "package.json"
[[manifest]]
path = "package.json"
kind = "json"
field = "version"
[[manifest]]
path = "Cargo.toml"
kind = "cargo-package"
[git]
require_clean_tree = false
"#,
        );
        std::process::Command::new("git")
            .current_dir(&dir)
            .args(["add", "."])
            .status()
            .unwrap();
        std::process::Command::new("git")
            .current_dir(&dir)
            .args(["commit", "-m", "chore: initial commit", "-q"])
            .status()
            .unwrap();
        std::process::Command::new("git")
            .current_dir(&dir)
            .args(["commit", "--allow-empty", "-m", "feat: exciting new feature", "-q"])
            .status()
            .unwrap();

        let args = Cli::try_parse_from([
            "cutver",
            "-c",
            &dir.join("release.toml").to_string_lossy(),
            "bump",
            "auto",
            "--dry-run",
        ])
        .unwrap();
        assert_eq!(run(args), 0);
    }

    #[test]
    fn config_defaults_and_preflight_order() {
        let dir = temp_dir("cutver-config-defaults");
        write(
            &dir,
            "release.toml",
            "[version]\ncurrent_source = \"a\"\n[[manifest]]\npath = \"a\"\nkind = \"cargo-package\"\n[preflight]\ntests = \"cargo test\"\nz = \"z\"\na = \"a\"\nm = \"m\"\n[changelog]\npath = \"CHANGELOG.md\"\n",
        );
        write(&dir, "a", "");
        let cfg = config::load(dir.join("release.toml")).unwrap();
        assert_eq!(Path::new(&cfg.version.current_source).file_name().unwrap(), "a");
        assert_eq!(
            (cfg.manifest.len(), cfg.preflight.len(), cfg.git.tag_prefix.as_str()),
            (1, 4, "v")
        );
        assert_eq!(
            (
                cfg.changelog.format.as_str(),
                cfg.changelog.entry_template.as_str(),
                cfg.git.commit_message.as_str()
            ),
            (
                "keep-a-changelog",
                "Maintenance and updates.",
                "chore(release): v{version}"
            )
        );
        assert!(cfg.git.require_clean_tree && cfg.git.require_branch.is_none());
        assert_eq!(
            cfg.preflight.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(),
            vec!["tests", "z", "a", "m"]
        );
    }

    #[test]
    fn changelog_latest_explicit_path() {
        let dir = temp_dir("cutver-cl-explicit");
        let cl = write(
            &dir,
            "MY_CHANGELOG.md",
            "# Changelog\n\n## [1.2.0] - 2026-03-01\n\n- Added feature X\n\n## [1.1.0] - 2026-02-01\n\n- Old feature\n",
        );
        let args = Cli::try_parse_from(["cutver", "changelog", "latest", "-p", &cl.to_string_lossy()]).unwrap();
        assert_eq!(run(args), 0);
    }

    #[test]
    fn changelog_latest_explicit_path_with_header() {
        let dir = temp_dir("cutver-cl-header");
        let cl = write(
            &dir,
            "MY_CHANGELOG.md",
            "# Changelog\n\n## [1.2.0] - 2026-03-01\n\n- Added feature X\n",
        );
        let args = Cli::try_parse_from(["cutver", "changelog", "latest", "-H", "-p", &cl.to_string_lossy()]).unwrap();
        assert_eq!(run(args), 0);
    }

    #[test]
    fn changelog_latest_from_config() {
        let dir = temp_dir("cutver-cl-cfg");
        write(
            &dir,
            "CUSTOM_CHANGELOG.md",
            "# Changelog\n\n## [2.0.0] - 2026-04-01\n\n- Breaking change\n",
        );
        write(&dir, "package.json", r#"{"version": "2.0.0"}"#);
        write(
            &dir,
            "cutver.toml",
            r#"
[version]
current_source = "package.json"
[[manifest]]
path = "package.json"
kind = "json"
field = "version"
[changelog]
path = "CUSTOM_CHANGELOG.md"
"#,
        );
        let args = Cli::try_parse_from([
            "cutver",
            "-c",
            &dir.join("cutver.toml").to_string_lossy(),
            "changelog",
            "latest",
        ])
        .unwrap();
        assert_eq!(run(args), 0);
    }

    #[test]
    fn changelog_latest_missing_explicit_path_fails() {
        let args = Cli::try_parse_from([
            "cutver",
            "changelog",
            "latest",
            "-p",
            "this-file-does-not-exist-12345.md",
        ])
        .unwrap();
        assert_eq!(run(args), 1);
    }

    #[test]
    fn changelog_latest_no_release_section_fails() {
        let dir = temp_dir("cutver-cl-no-rel");
        let cl = write(
            &dir,
            "CHANGELOG.md",
            "# Changelog\n\n## [Unreleased]\n\n- Work in progress\n",
        );
        let args = Cli::try_parse_from(["cutver", "changelog", "latest", "-p", &cl.to_string_lossy()]).unwrap();
        assert_eq!(run(args), 1);
    }

    #[test]
    fn changelog_latest_config_missing_file_fails() {
        let dir = temp_dir("cutver-cl-cfg-missing");
        write(&dir, "package.json", r#"{"version": "1.0.0"}"#);
        write(
            &dir,
            "cutver.toml",
            r#"
[version]
current_source = "package.json"
[[manifest]]
path = "package.json"
kind = "json"
field = "version"
[changelog]
path = "NONEXISTENT.md"
"#,
        );
        let args = Cli::try_parse_from([
            "cutver",
            "-c",
            &dir.join("cutver.toml").to_string_lossy(),
            "changelog",
            "latest",
        ])
        .unwrap();
        assert_eq!(run(args), 1);
    }

    #[test]
    fn changelog_latest_config_without_changelog_path_fallback() {
        let dir = temp_dir("cutver-cl-no-path");
        write(
            &dir,
            "CHANGELOG.md",
            "# Changelog\n\n## [1.0.0] - 2026-01-01\n\n- Initial release\n",
        );
        write(&dir, "package.json", r#"{"version": "1.0.0"}"#);
        write(
            &dir,
            "cutver.toml",
            r#"
[version]
current_source = "package.json"
[[manifest]]
path = "package.json"
kind = "json"
field = "version"
"#,
        );
        let args = Cli::try_parse_from([
            "cutver",
            "-c",
            &dir.join("cutver.toml").to_string_lossy(),
            "changelog",
            "latest",
        ])
        .unwrap();
        assert_eq!(run(args), 0);
    }

    #[test]
    fn run_changelog_direct() {
        let dir = temp_dir("cutver-run-cl");
        let cl = write(
            &dir,
            "CHANGELOG.md",
            "# Changelog\n\n## [1.0.0] - 2026-01-01\n\n- Released\n",
        );
        let cmd = ChangelogCommands::Latest {
            include_header: false,
            path: Some(cl),
        };
        assert_eq!(run_changelog(None, cmd), 0);
    }

    #[test]
    fn changelog_show_explicit_path() {
        let dir = temp_dir("cutver-cl-show-explicit");
        let cl = write(
            &dir,
            "MY_CHANGELOG.md",
            "# Changelog\n\n## [1.2.0] - 2026-03-01\n\n- Added feature X\n\n## [1.1.0] - 2026-02-01\n\n- Old feature\n",
        );
        let args = Cli::try_parse_from(["cutver", "changelog", "show", "1.1.0", "-p", &cl.to_string_lossy()]).unwrap();
        assert_eq!(run(args), 0);
    }

    #[test]
    fn changelog_show_explicit_path_with_header() {
        let dir = temp_dir("cutver-cl-show-header");
        let cl = write(
            &dir,
            "MY_CHANGELOG.md",
            "# Changelog\n\n## [1.2.0] - 2026-03-01\n\n- Added feature X\n\n## [1.1.0] - 2026-02-01\n\n- Old feature\n",
        );
        let args = Cli::try_parse_from([
            "cutver",
            "changelog",
            "show",
            "1.1.0",
            "-H",
            "-p",
            &cl.to_string_lossy(),
        ])
        .unwrap();
        assert_eq!(run(args), 0);
    }

    #[test]
    fn changelog_show_missing_version_fails() {
        let dir = temp_dir("cutver-cl-show-missing");
        let cl = write(
            &dir,
            "MY_CHANGELOG.md",
            "# Changelog\n\n## [1.2.0] - 2026-03-01\n\n- Added feature X\n",
        );
        let args = Cli::try_parse_from(["cutver", "changelog", "show", "0.9.0", "-p", &cl.to_string_lossy()]).unwrap();
        assert_eq!(run(args), 1);
    }

    #[test]
    fn run_changelog_show_direct() {
        let dir = temp_dir("cutver-run-cl-show");
        let cl = write(
            &dir,
            "CHANGELOG.md",
            "# Changelog\n\n## [1.0.0] - 2026-01-01\n\n- Released\n",
        );
        let cmd = ChangelogCommands::Show {
            version: "1.0.0".to_string(),
            include_header: false,
            path: Some(cl),
        };
        assert_eq!(run_changelog(None, cmd), 0);
    }
}
