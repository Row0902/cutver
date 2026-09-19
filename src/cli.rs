use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

/// Cut a release. Bump SemVer. Every project, every language.
#[derive(Parser, Debug)]
#[command(name = "cutver", version, about)]
pub struct Cli {
    /// Path to release.toml (defaults to discovering release.toml walking up from current directory)
    #[arg(short, long, global = true, value_name = "PATH")]
    pub config: Option<PathBuf>,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Synchronize versions across manifests, run preflight checks, update changelog, and create a Git commit and tag
    Bump {
        /// SemVer level to increment
        #[arg(value_enum)]
        level: BumpLevel,
        /// Simulate the release pipeline without modifying files or creating Git commits/tags
        #[arg(long)]
        dry_run: bool,
        /// Skip named preflight verification steps (repeatable)
        #[arg(long, value_name = "STEP")]
        skip_preflight: Vec<String>,
    },
    /// Validate release.toml configuration and report version drift across declared manifests
    Doctor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum BumpLevel {
    /// Increment patch version (e.g. 1.2.3 -> 1.2.4)
    Patch,
    /// Increment minor version (e.g. 1.2.3 -> 1.3.0)
    Minor,
    /// Increment major version (e.g. 1.2.3 -> 2.0.0)
    Major,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bump_patch_defaults() {
        let cli = Cli::try_parse_from(["cutver", "bump", "patch"]).unwrap();
        let Commands::Bump {
            level,
            dry_run,
            skip_preflight,
        } = cli.command
        else {
            panic!("expected bump")
        };
        assert_eq!(level, BumpLevel::Patch);
        assert!(!dry_run);
        assert!(skip_preflight.is_empty());
        assert!(cli.config.is_none());
    }

    #[test]
    fn bump_minor_dry_run_and_skip_preflight() {
        let cli = Cli::try_parse_from([
            "cutver",
            "bump",
            "minor",
            "--dry-run",
            "--skip-preflight",
            "tests",
            "--skip-preflight",
            "build",
        ])
        .unwrap();
        let Commands::Bump {
            level,
            dry_run,
            skip_preflight,
        } = cli.command
        else {
            panic!("expected bump")
        };
        assert_eq!(level, BumpLevel::Minor);
        assert!(dry_run);
        assert_eq!(skip_preflight, vec!["tests", "build"]);
    }

    #[test]
    fn doctor_subcommand() {
        let cli = Cli::try_parse_from(["cutver", "doctor"]).unwrap();
        assert!(matches!(cli.command, Commands::Doctor));
    }

    #[test]
    fn config_override_global() {
        let cli = Cli::try_parse_from(["cutver", "-c", "other.toml", "doctor"]).unwrap();
        assert_eq!(cli.config, Some(PathBuf::from("other.toml")));
    }
}
