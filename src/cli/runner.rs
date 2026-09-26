use std::path::{Path, PathBuf};

use crate::bump::{self, Drift, Summary};
use crate::cli::args::{BumpLevel, ChangelogCommands, Cli, Commands};
use crate::config;

pub fn run(args: Cli) -> i32 {
    match args.command {
        Commands::Init {
            update,
            force,
            path,
            no_template,
        } => match crate::init::run_init(path, update, force, no_template) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("Error: {e}");
                1
            }
        },
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
            first_release,
        } => {
            let config = match load_config(args.config) {
                Ok(c) => c,
                Err(code) => return code,
            };
            run_bump(&config, level, dry_run, &skip_preflight, first_release)
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

fn load_template_file(config_override: Option<&Path>, template_path: &Path) -> Result<String, String> {
    let resolved = if template_path.is_absolute() {
        template_path.to_path_buf()
    } else {
        let cfg = config_override
            .and_then(|p| config::load(p).ok())
            .or_else(|| std::env::current_dir().ok().and_then(|d| config::discover(d).ok()));
        if let Some(ref c) = cfg {
            let candidate = c.root_dir.join(template_path);
            if candidate.is_file() {
                candidate
            } else {
                template_path.to_path_buf()
            }
        } else {
            template_path.to_path_buf()
        }
    };

    std::fs::read_to_string(&resolved)
        .map_err(|e| format!("failed to read template file '{}': {e}", template_path.display()))
}

fn extract_heading_date(content: &str, target_ver: &str) -> Option<String> {
    let target_norm = target_ver.trim_start_matches(['v', 'V']);
    for line in content.lines() {
        if let Some(heading) = line.strip_prefix("## ") {
            let after_h2 = heading.trim_start();
            let lower = after_h2.to_ascii_lowercase();
            if lower.starts_with("[unreleased]") || lower.starts_with("unreleased") {
                continue;
            }
            if let Some((ver_part, date_part)) = after_h2.split_once(" - ") {
                let v = ver_part.trim().trim_matches(['[', ']']).trim_start_matches(['v', 'V']);
                if v == target_norm {
                    let d = date_part.trim();
                    if !d.is_empty() {
                        return Some(d.to_string());
                    }
                }
            }
        }
    }
    None
}

fn find_raw_heading_version<'a>(content: &'a str, target_ver: &str) -> Option<&'a str> {
    let target_norm = target_ver.trim_start_matches(['v', 'V']);
    for line in content.lines() {
        if let Some(heading) = line.strip_prefix("## ") {
            let after_h2 = heading.trim_start();
            let lower = after_h2.to_ascii_lowercase();
            if lower.starts_with("[unreleased]") || lower.starts_with("unreleased") {
                continue;
            }
            let raw_ver = if after_h2.starts_with('[') {
                after_h2.find(']').map(|end| after_h2[1..end].trim())
            } else {
                after_h2
                    .split_whitespace()
                    .next()
                    .map(|s| s.trim_end_matches(':').trim())
            };
            if let Some(raw) = raw_ver
                && raw.trim_start_matches(['v', 'V']) == target_norm
            {
                return Some(raw);
            }
        }
    }
    None
}

fn resolve_release_tag_and_prefix(
    cfg: Option<&config::Config>,
    root_dir: &Path,
    content: &str,
    version: &str,
) -> (String, String) {
    let clean_ver = version.trim_start_matches(['v', 'V']);
    if let Some(c) = cfg {
        let prefix = c.git.tag_prefix.clone();
        let tag = format!("{prefix}{clean_ver}");
        return (tag, prefix);
    }

    let v_tag = format!("v{clean_ver}");
    if crate::git::tag_exists(root_dir, &v_tag).unwrap_or(false) {
        return (v_tag, "v".to_string());
    }
    if crate::git::tag_exists(root_dir, clean_ver).unwrap_or(false) {
        return (clean_ver.to_string(), String::new());
    }

    if let Some(raw) = find_raw_heading_version(content, clean_ver)
        && raw.starts_with(['v', 'V'])
    {
        return (format!("v{clean_ver}"), "v".to_string());
    }

    (clean_ver.to_string(), String::new())
}

pub fn run_changelog(config_override: Option<&Path>, command: ChangelogCommands) -> i32 {
    match command {
        ChangelogCommands::Latest {
            include_header,
            path,
            template,
        } => {
            let target_path = match resolve_changelog_path(config_override, path) {
                Ok(p) => p,
                Err(code) => return code,
            };

            if let Some(ref t_path) = template {
                let template_str = match load_template_file(config_override, t_path) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("Error: {e}");
                        return 1;
                    }
                };

                let content = match std::fs::read_to_string(&target_path) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("Error: failed to read changelog '{}': {e}", target_path.display());
                        return 1;
                    }
                };

                let versions = crate::changelog::list_versions(&content);
                if versions.is_empty() {
                    eprintln!(
                        "Error: no release section found in changelog '{}'",
                        target_path.display()
                    );
                    return 1;
                }

                let latest_ver = &versions[0];
                let prev_ver = versions.get(1).map(String::as_str);

                let cfg = config_override
                    .and_then(|p| config::load(p).ok())
                    .or_else(|| std::env::current_dir().ok().and_then(|d| config::discover(d).ok()));

                let include_scopes = cfg.as_ref().map(|c| c.changelog.include_scopes).unwrap_or(true);
                let fallback_entry = cfg
                    .as_ref()
                    .map(|c| c.changelog.fallback_entry.as_str())
                    .unwrap_or("Maintenance and updates.");
                let dummy_root = std::path::PathBuf::from(".");
                let root_dir = cfg
                    .as_ref()
                    .map(|c| c.root_dir.as_path())
                    .or_else(|| target_path.parent())
                    .unwrap_or(&dummy_root);

                let (tag, _) = resolve_release_tag_and_prefix(cfg.as_ref(), root_dir, &content, latest_ver);
                let prev_tag =
                    prev_ver.map(|pv| resolve_release_tag_and_prefix(cfg.as_ref(), root_dir, &content, pv).0);

                let commits_raw = crate::git::commits_since(root_dir, prev_tag.as_deref()).unwrap_or_default();
                let (_bump, parsed_commits) = crate::conventional::parse_and_deduce_bump(&commits_raw);
                let contributors = crate::git::list_authors_since(root_dir, prev_tag.as_deref()).unwrap_or_default();
                let repository = crate::git::remote_url(root_dir);
                let date_str = extract_heading_date(&content, latest_ver)
                    .unwrap_or_else(|| crate::changelog::format_date(std::time::SystemTime::now()));

                let mut ctx = crate::changelog::build_context(
                    latest_ver,
                    prev_ver,
                    &tag,
                    prev_tag.as_deref(),
                    &date_str,
                    repository,
                    &parsed_commits,
                    contributors,
                    include_scopes,
                    fallback_entry,
                );

                if parsed_commits.is_empty()
                    && let Some(body) = crate::changelog::extract_latest(&content, false)
                    && !body.is_empty()
                {
                    ctx.all_changes = body;
                }

                match crate::changelog::render_template(&template_str, &ctx) {
                    Ok(rendered) => {
                        println!("{rendered}");
                        0
                    }
                    Err(e) => {
                        eprintln!("Error: {e}");
                        1
                    }
                }
            } else {
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
        }
        ChangelogCommands::Show {
            version,
            include_header,
            path,
            template,
        } => {
            let target_path = match resolve_changelog_path(config_override, path) {
                Ok(p) => p,
                Err(code) => return code,
            };

            if let Some(ref t_path) = template {
                let template_str = match load_template_file(config_override, t_path) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("Error: {e}");
                        return 1;
                    }
                };

                let content = match std::fs::read_to_string(&target_path) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("Error: failed to read changelog '{}': {e}", target_path.display());
                        return 1;
                    }
                };

                let target_norm = version.trim_start_matches(['v', 'V']);
                let versions = crate::changelog::list_versions(&content);
                let pos = versions
                    .iter()
                    .position(|v| v.trim_start_matches(['v', 'V']) == target_norm);
                let idx = match pos {
                    Some(i) => i,
                    None => {
                        eprintln!(
                            "Error: version '{version}' not found in changelog '{}'",
                            target_path.display()
                        );
                        return 1;
                    }
                };

                let matched_ver = &versions[idx];
                let prev_ver = versions.get(idx + 1).map(String::as_str);

                let cfg = config_override
                    .and_then(|p| config::load(p).ok())
                    .or_else(|| std::env::current_dir().ok().and_then(|d| config::discover(d).ok()));

                let include_scopes = cfg.as_ref().map(|c| c.changelog.include_scopes).unwrap_or(true);
                let fallback_entry = cfg
                    .as_ref()
                    .map(|c| c.changelog.fallback_entry.as_str())
                    .unwrap_or("Maintenance and updates.");
                let dummy_root = std::path::PathBuf::from(".");
                let root_dir = cfg
                    .as_ref()
                    .map(|c| c.root_dir.as_path())
                    .or_else(|| target_path.parent())
                    .unwrap_or(&dummy_root);

                let (tag, _) = resolve_release_tag_and_prefix(cfg.as_ref(), root_dir, &content, matched_ver);
                let prev_tag =
                    prev_ver.map(|pv| resolve_release_tag_and_prefix(cfg.as_ref(), root_dir, &content, pv).0);

                let commits_raw = crate::git::commits_between(root_dir, prev_tag.as_deref(), &tag).unwrap_or_default();
                let (_bump, parsed_commits) = crate::conventional::parse_and_deduce_bump(&commits_raw);
                let contributors =
                    crate::git::list_authors_between(root_dir, prev_tag.as_deref(), &tag).unwrap_or_default();
                let repository = crate::git::remote_url(root_dir);
                let date_str = extract_heading_date(&content, matched_ver)
                    .unwrap_or_else(|| crate::changelog::format_date(std::time::SystemTime::now()));

                let mut ctx = crate::changelog::build_context(
                    matched_ver,
                    prev_ver,
                    &tag,
                    prev_tag.as_deref(),
                    &date_str,
                    repository,
                    &parsed_commits,
                    contributors,
                    include_scopes,
                    fallback_entry,
                );

                if parsed_commits.is_empty()
                    && let Some(body) = crate::changelog::extract_version(&content, &version, false)
                    && !body.is_empty()
                {
                    ctx.all_changes = body;
                }

                match crate::changelog::render_template(&template_str, &ctx) {
                    Ok(rendered) => {
                        println!("{rendered}");
                        0
                    }
                    Err(e) => {
                        eprintln!("Error: {e}");
                        1
                    }
                }
            } else {
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
}

pub fn run_bump(
    config: &config::Config,
    level: BumpLevel,
    dry_run: bool,
    skip_preflight: &[String],
    first_release: bool,
) -> i32 {
    match bump::run_with_first_release(config, level, dry_run, skip_preflight, first_release) {
        Ok(summary) => {
            print_bump_summary(&summary);
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
                eprintln!("✖ Version drift detected ({} manifest(s) out of sync):", drifts.len());
                for Drift { path, expected, actual } in drifts {
                    eprintln!("  • {path}: expected {expected}, found {actual}");
                }
                eprintln!("\nSuggested fix:");
                eprintln!("  • Align version numbers manually across all files.");
                eprintln!("  • Or run 'cutver bump patch' to synchronize all manifests in one step.");
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
                    eprintln!("✖ Changelog drift detected:");
                    if !cl_drift.missing_in_changelog.is_empty() {
                        eprintln!("\n  Missing in changelog (Git tag exists):");
                        for tag in &cl_drift.missing_in_changelog {
                            eprintln!("    • {tag}");
                        }
                    }
                    if !cl_drift.orphan_sections.is_empty() {
                        eprintln!("\n  Orphan changelog sections (no Git tag exists):");
                        for sec in &cl_drift.orphan_sections {
                            eprintln!("    • {sec}");
                        }
                    }
                    eprintln!("\nSuggested next steps:");
                    eprintln!(
                        "  • Add missing release sections to CHANGELOG.md or query with 'cutver changelog show <version>'."
                    );
                    eprintln!("  • Check if orphan versions were tagged with a different prefix or not yet pushed.");
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
    println!("✔ {filename} is valid.");
    println!("  manifests: {}", config.manifest.len());
    println!("  preflight steps: {}", config.preflight.len());
    println!("  current source: {}", config.version.current_source);
    if check_changelog {
        println!("  changelog: consistent with Git tags");
    }
    0
}

pub fn print_bump_summary(summary: &Summary) {
    if summary.dry_run {
        println!("ℹ Running in simulation mode (--dry-run). No files, commits, or tags will be modified.\n");
        println!("Release Plan:");
    } else {
        println!("✔ Release Plan:");
    }
    println!("  source: {}", summary.source);
    println!("  current version: {}", summary.current);
    println!("  next version: {}", summary.next);
    println!("  dry run: {}", summary.dry_run);
    println!("  preflight commands:");
    for step in &summary.preflight {
        let marker = if step.skipped { " [SKIPPED]" } else { "" };
        println!("    • {}: {}{}", step.name, step.command, marker);
    }
    println!("  manifests:");
    for t in &summary.touched {
        println!("    • {}: {} -> {}", t.path, t.old, t.new);
    }
    if let Some(cl) = &summary.changelog {
        println!("  changelog: {cl}");
    }
    if let Some(hook) = &summary.post_bump {
        println!("  post_bump: {hook}");
    }
    println!("  commit: {}", summary.commit_message);
    println!("  tag: {}", summary.tag);
    if let Some(ft) = &summary.floating_tag {
        println!("  floating tag: {ft}");
    }
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
                println!("      • {cmd}");
            }
        }
    }
}

#[allow(dead_code)]
pub fn print_summary(summary: &Summary) {
    print_bump_summary(summary);
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
            ("keep-a-changelog", "", "chore(release): v{version}")
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
            template: None,
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
            template: None,
        };
        assert_eq!(run_changelog(None, cmd), 0);
    }

    #[test]
    fn run_init_success_and_fail_on_collision() {
        let dir = temp_dir("cutver-runner-init");
        write(&dir, "Cargo.toml", "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n");

        let args = Cli::try_parse_from(["cutver", "init", "-p", &dir.to_string_lossy()]).unwrap();
        assert_eq!(run(args), 0);
        assert!(dir.join("cutver.toml").is_file());

        // Second run without force should fail
        let args_fail = Cli::try_parse_from(["cutver", "init", "-p", &dir.to_string_lossy()]).unwrap();
        assert_eq!(run(args_fail), 1);

        // Third run with force should succeed
        let args_force = Cli::try_parse_from(["cutver", "init", "-p", &dir.to_string_lossy(), "--force"]).unwrap();
        assert_eq!(run(args_force), 0);
    }
}
