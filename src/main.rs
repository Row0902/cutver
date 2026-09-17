use clap::Parser;
use cutver::cli::{BumpLevel, Cli, Commands};
use cutver::config::Config;
use cutver::semver_bump::{Bump, bump};
use std::path::PathBuf;
use std::process;

fn main() {
    if let Err(e) = run(Cli::parse()) {
        eprintln!("Error: {e:?}");
        process::exit(1);
    }
}

fn run(args: Cli) -> Result<(), Box<dyn std::error::Error>> {
    let config_path = args.config.unwrap_or_else(|| PathBuf::from("release.toml"));
    let config = cutver::config::load(&config_path)?;

    match args.command {
        Commands::Doctor => {
            doctor(&config);
            Ok(())
        }
        Commands::Bump {
            level,
            dry_run,
            skip_preflight,
        } => bump_stub(&config, level, dry_run, &skip_preflight),
    }
}

fn doctor(config: &Config) {
    println!("release.toml is valid.");
    println!("  manifests: {}", config.manifest.len());
    println!("  preflight steps: {}", config.preflight.len());
    println!("  current source: {}", config.version.current_source);
}

fn bump_stub(
    config: &Config,
    level: BumpLevel,
    dry_run: bool,
    skip_preflight: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    let bump_kind = match level {
        BumpLevel::Patch => Bump::Patch,
        BumpLevel::Minor => Bump::Minor,
        BumpLevel::Major => Bump::Major,
    };

    let source_entry = config
        .manifest
        .iter()
        .find(|m| m.path == config.version.current_source)
        .ok_or_else(|| msg("current_source manifest entry not found"))?;
    let content = std::fs::read_to_string(&source_entry.path)?;
    let editor = cutver::manifest::editor_for(source_entry)?;
    let current = editor.read_version(&content)?;
    let next = bump(&current, bump_kind);

    println!("Bump summary:");
    println!("  source: {}", source_entry.path);
    println!("  current version: {}", current);
    println!("  next version: {}", next);
    println!("  dry run: {}", dry_run);
    println!("  manifests to touch:");
    for m in &config.manifest {
        let marker = if m.path == source_entry.path {
            " (source)"
        } else {
            ""
        };
        println!("    - {}{}", m.path, marker);
    }
    println!("  preflight commands:");
    for (name, cmd) in &config.preflight {
        let skipped = skip_preflight.contains(name);
        println!(
            "    - {}: {}{}",
            name,
            cmd,
            if skipped { " [SKIPPED]" } else { "" }
        );
    }
    Ok(())
}

fn msg(s: &str) -> Box<dyn std::error::Error> {
    Box::new(std::io::Error::new(
        std::io::ErrorKind::Other,
        s.to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir(prefix: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("{}-{}", prefix, std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_file(dir: &PathBuf, name: &str, text: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, text).unwrap();
        path
    }

    #[test]
    fn doctor_reports_valid_config() {
        let dir = temp_dir("cutver-doc-ok");
        write_file(
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
        write_file(&dir, "package.json", r#"{"version": "1.0.0"}"#);
        let args = Cli::try_parse_from([
            "cutver",
            "-c",
            &dir.join("release.toml").to_string_lossy(),
            "doctor",
        ])
        .unwrap();
        assert!(run(args).is_ok());
    }

    #[test]
    fn doctor_fails_for_invalid_config() {
        let dir = temp_dir("cutver-doc-bad");
        write_file(
            &dir,
            "release.toml",
            "[version]\ncurrent_source = \"missing\"",
        );
        let args = Cli::try_parse_from([
            "cutver",
            "-c",
            &dir.join("release.toml").to_string_lossy(),
            "doctor",
        ])
        .unwrap();
        assert!(run(args).is_err());
    }

    #[test]
    fn bump_stub_reports_summary_without_mutating() {
        let dir = temp_dir("cutver-bump-stub");
        let pkg = dir.join("package.json");
        let cargo = dir.join("Cargo.toml");
        let cfg = format!(
            r#"
[version]
current_source = "{pkg}"
[[manifest]]
path = "{pkg}"
kind = "json"
field = "version"
[[manifest]]
path = "{cargo}"
kind = "cargo-package"
[preflight]
tests = "cargo test"
"#,
            pkg = pkg.to_string_lossy(),
            cargo = cargo.to_string_lossy(),
        );
        write_file(&dir, "release.toml", &cfg);
        write_file(&dir, "package.json", r#"{"version": "1.2.3"}"#);
        let cargo_before = r#"[package]
version = "1.2.3"
"#;
        write_file(&dir, "Cargo.toml", cargo_before);
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
        assert!(run(args).is_ok());
        assert_eq!(fs::read_to_string(&cargo).unwrap(), cargo_before);
    }
}
