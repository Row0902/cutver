use clap::Parser;
use cutver::bump::{self, Drift, Summary};
use cutver::cli::{BumpLevel, Cli, Commands};
use cutver::config;
use cutver::semver_bump::Bump;
use std::process;

fn main() {
    let code = run(Cli::parse());
    if code != 0 {
        process::exit(code);
    }
}

fn run(args: Cli) -> i32 {
    let config = match args.config {
        Some(path) => config::load(path),
        None => {
            let start_dir = match std::env::current_dir() {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("Error: unable to determine current directory: {e}");
                    return 1;
                }
            };
            config::discover(start_dir)
        }
    };
    match config {
        Ok(config) => match args.command {
            Commands::Doctor => run_doctor(&config),
            Commands::Bump {
                level,
                dry_run,
                skip_preflight,
            } => run_bump(&config, level, dry_run, &skip_preflight),
        },
        Err(e) => {
            eprintln!("Error loading config: {e}");
            1
        }
    }
}

fn run_bump(config: &config::Config, level: BumpLevel, dry_run: bool, skip_preflight: &[String]) -> i32 {
    let kind = match level {
        BumpLevel::Patch => Bump::Patch,
        BumpLevel::Minor => Bump::Minor,
        BumpLevel::Major => Bump::Major,
    };
    match bump::run(config, kind, dry_run, skip_preflight) {
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

fn run_doctor(config: &config::Config) -> i32 {
    match bump::doctor(config) {
        Ok(drifts) if drifts.is_empty() => {
            println!("release.toml is valid.");
            println!("  manifests: {}", config.manifest.len());
            println!("  preflight steps: {}", config.preflight.len());
            println!("  current source: {}", config.version.current_source);
            0
        }
        Ok(drifts) => {
            eprintln!("Drift detected ({} manifest(s) out of sync):", drifts.len());
            for Drift { path, expected, actual } in drifts {
                eprintln!("  - {path}: expected {expected}, found {actual}");
            }
            2
        }
        Err(e) => {
            eprintln!("Error: {e}");
            1
        }
    }
}

fn print_summary(summary: &Summary) {
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
    println!("  commit: {}", summary.commit_message);
    println!("  tag: {}", summary.tag);
}

#[cfg(test)]
mod tests {
    static TMP_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    fn tmp_id(prefix: &str) -> String {
        let n = TMP_SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        format!("{}-{}-{}", prefix, std::process::id(), n)
    }

    use super::*;
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
    fn bump_dry_run_does_not_mutate() {
        let dir = temp_dir("cutver-bump-stub");
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
    fn config_defaults_and_preflight_order() {
        let dir = temp_dir("cutver-config-defaults");
        write(
            &dir,
            "release.toml",
            "[version]\ncurrent_source = \"a\"\n[[manifest]]\npath = \"a\"\nkind = \"cargo-package\"\n[preflight]\ntests = \"cargo test\"\nz = \"z\"\na = \"a\"\nm = \"m\"\n[changelog]\npath = \"CHANGELOG.md\"\n",
        );
        write(&dir, "a", "");
        let cfg = config::load(dir.join("release.toml")).unwrap();
        assert_eq!(cfg.version.current_source, dir.join("a").to_string_lossy().to_string());
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
}
