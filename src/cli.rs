use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

/// Cut a release. Bump SemVer. Every project, every language.
#[derive(Parser, Debug)]
#[command(name = "cutver", version, about)]
pub struct Cli {
    /// Path to configuration file (defaults to discovering cutver.toml or release.toml walking up from current directory)
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
    /// Query or extract entries from the changelog
    Changelog {
        #[command(subcommand)]
        command: ChangelogCommands,
    },
}

#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum ChangelogCommands {
    /// Extract the latest release notes from the changelog
    Latest {
        /// Include the release heading (e.g. '## [0.3.1] - 2026-09-20')
        #[arg(short = 'H', long)]
        include_header: bool,
        /// Explicit path to changelog file (defaults to changelog configured in cutver.toml or CHANGELOG.md)
        #[arg(short, long, value_name = "PATH")]
        path: Option<PathBuf>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum BumpLevel {
    /// Increment patch version (e.g. 1.2.3 -> 1.2.4)
    Patch,
    /// Increment minor version (e.g. 1.2.3 -> 1.3.0)
    Minor,
    /// Increment major version (e.g. 1.2.3 -> 2.0.0)
    Major,
    /// Automatically deduce bump level from Conventional Commits since the latest tag
    Auto,
}

impl From<crate::semver_bump::Bump> for BumpLevel {
    fn from(b: crate::semver_bump::Bump) -> Self {
        match b {
            crate::semver_bump::Bump::Patch => BumpLevel::Patch,
            crate::semver_bump::Bump::Minor => BumpLevel::Minor,
            crate::semver_bump::Bump::Major => BumpLevel::Major,
        }
    }
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
    fn bump_auto_cli() {
        let cli = Cli::try_parse_from(["cutver", "bump", "auto"]).unwrap();
        let Commands::Bump {
            level,
            dry_run,
            skip_preflight,
        } = cli.command
        else {
            panic!("expected bump")
        };
        assert_eq!(level, BumpLevel::Auto);
        assert!(!dry_run);
        assert!(skip_preflight.is_empty());
    }

    #[test]
    fn bump_auto_dry_run() {
        let cli = Cli::try_parse_from(["cutver", "bump", "auto", "--dry-run"]).unwrap();
        let Commands::Bump { level, dry_run, .. } = cli.command else {
            panic!("expected bump")
        };
        assert_eq!(level, BumpLevel::Auto);
        assert!(dry_run);
    }

    #[test]
    fn doctor_subcommand() {
        let cli = Cli::try_parse_from(["cutver", "doctor"]).unwrap();
        assert!(matches!(cli.command, Commands::Doctor));
    }

    #[test]
    fn changelog_latest_defaults() {
        let cli = Cli::try_parse_from(["cutver", "changelog", "latest"]).unwrap();
        let Commands::Changelog {
            command: ChangelogCommands::Latest { include_header, path },
        } = cli.command
        else {
            panic!("expected changelog latest");
        };
        assert!(!include_header);
        assert!(path.is_none());
    }

    #[test]
    fn changelog_latest_with_options() {
        let cli = Cli::try_parse_from([
            "cutver",
            "changelog",
            "latest",
            "--include-header",
            "--path",
            "docs/HISTORY.md",
        ])
        .unwrap();
        let Commands::Changelog { command } = cli.command else {
            panic!("expected changelog");
        };
        assert_eq!(
            command,
            ChangelogCommands::Latest {
                include_header: true,
                path: Some(PathBuf::from("docs/HISTORY.md")),
            }
        );

        let cli_short = Cli::try_parse_from(["cutver", "changelog", "latest", "-H", "-p", "custom.md"]).unwrap();
        let Commands::Changelog { command: command_short } = cli_short.command else {
            panic!("expected changelog");
        };
        assert_eq!(
            command_short,
            ChangelogCommands::Latest {
                include_header: true,
                path: Some(PathBuf::from("custom.md")),
            }
        );
    }

    #[test]
    fn config_override_global() {
        let cli = Cli::try_parse_from(["cutver", "-c", "other.toml", "doctor"]).unwrap();
        assert_eq!(cli.config, Some(PathBuf::from("other.toml")));
    }
}
