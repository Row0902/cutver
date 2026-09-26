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
    /// Initialize a new cutver.toml configuration by discovering project manifests
    Init {
        /// Update an existing cutver.toml with newly discovered manifests without overwriting settings
        #[arg(short, long)]
        update: bool,
        /// Overwrite existing configuration file if present
        #[arg(short, long)]
        force: bool,
        /// Target directory to inspect and initialize (defaults to current directory)
        #[arg(short, long, value_name = "DIR")]
        path: Option<PathBuf>,
    },
    /// Synchronize versions across manifests, run preflight checks, update changelog, and create a Git commit and tag
    Bump {
        /// SemVer level to increment
        #[arg(value_enum, default_value = "auto")]
        level: BumpLevel,
        /// Simulate the release pipeline without modifying files or creating Git commits/tags
        #[arg(long)]
        dry_run: bool,
        /// Skip named preflight verification steps (repeatable)
        #[arg(long, value_name = "STEP")]
        skip_preflight: Vec<String>,
        /// Initial release without incrementing the version in manifests
        #[arg(long = "first-release", visible_alias = "fr")]
        first_release: bool,
    },
    /// Validate configuration and report version drift across declared manifests
    Doctor {
        /// Also validate that CHANGELOG.md is consistent with Git release tags
        #[arg(long)]
        check_changelog: bool,
    },
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
        /// Optional path to an arbitrary MiniJinja template file to format the output
        #[arg(long, value_name = "PATH")]
        template: Option<PathBuf>,
    },
    /// Extract release notes for a specific version from the changelog
    Show {
        /// The version to extract (e.g. '0.2.0' or 'v0.2.0')
        #[arg(value_name = "VERSION")]
        version: String,
        /// Include the release heading (e.g. '## [0.2.0] - 2026-09-19')
        #[arg(short = 'H', long)]
        include_header: bool,
        /// Explicit path to changelog file (defaults to changelog configured in cutver.toml or CHANGELOG.md)
        #[arg(short, long, value_name = "PATH")]
        path: Option<PathBuf>,
        /// Optional path to an arbitrary MiniJinja template file to format the output
        #[arg(long, value_name = "PATH")]
        template: Option<PathBuf>,
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

pub fn normalize_args<I, T>(args: I) -> Vec<String>
where
    I: IntoIterator<Item = T>,
    T: Into<String>,
{
    args.into_iter()
        .map(|arg| {
            let s = arg.into();
            if s == "-fr" { "--first-release".to_string() } else { s }
        })
        .collect()
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
            first_release,
        } = cli.command
        else {
            panic!("expected bump")
        };
        assert_eq!(level, BumpLevel::Patch);
        assert!(!dry_run);
        assert!(skip_preflight.is_empty());
        assert!(!first_release);
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
            first_release,
        } = cli.command
        else {
            panic!("expected bump")
        };
        assert_eq!(level, BumpLevel::Minor);
        assert!(dry_run);
        assert_eq!(skip_preflight, vec!["tests", "build"]);
        assert!(!first_release);
    }

    #[test]
    fn bump_auto_cli() {
        let cli = Cli::try_parse_from(["cutver", "bump", "auto"]).unwrap();
        let Commands::Bump {
            level,
            dry_run,
            skip_preflight,
            first_release,
        } = cli.command
        else {
            panic!("expected bump")
        };
        assert_eq!(level, BumpLevel::Auto);
        assert!(!dry_run);
        assert!(skip_preflight.is_empty());
        assert!(!first_release);
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
        assert!(matches!(cli.command, Commands::Doctor { .. }));
    }

    #[test]
    fn doctor_defaults() {
        let cli = Cli::try_parse_from(["cutver", "doctor"]).unwrap();
        let Commands::Doctor { check_changelog } = cli.command else {
            panic!("expected doctor");
        };
        assert!(!check_changelog);
    }

    #[test]
    fn doctor_with_check_changelog() {
        let cli = Cli::try_parse_from(["cutver", "doctor", "--check-changelog"]).unwrap();
        let Commands::Doctor { check_changelog } = cli.command else {
            panic!("expected doctor");
        };
        assert!(check_changelog);
    }

    #[test]
    fn changelog_latest_defaults() {
        let cli = Cli::try_parse_from(["cutver", "changelog", "latest"]).unwrap();
        let Commands::Changelog {
            command:
                ChangelogCommands::Latest {
                    include_header,
                    path,
                    template,
                },
        } = cli.command
        else {
            panic!("expected changelog latest");
        };
        assert!(!include_header);
        assert!(path.is_none());
        assert!(template.is_none());
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
            "--template",
            "release.j2",
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
                template: Some(PathBuf::from("release.j2")),
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
                template: None,
            }
        );
    }

    #[test]
    fn changelog_show_defaults() {
        let cli = Cli::try_parse_from(["cutver", "changelog", "show", "0.2.0"]).unwrap();
        let Commands::Changelog {
            command:
                ChangelogCommands::Show {
                    version,
                    include_header,
                    path,
                    template,
                },
        } = cli.command
        else {
            panic!("expected changelog show");
        };
        assert_eq!(version, "0.2.0");
        assert!(!include_header);
        assert!(path.is_none());
        assert!(template.is_none());
    }

    #[test]
    fn changelog_show_with_options() {
        let cli = Cli::try_parse_from([
            "cutver",
            "changelog",
            "show",
            "v1.0.0",
            "--include-header",
            "--path",
            "docs/HISTORY.md",
            "--template",
            "notes.j2",
        ])
        .unwrap();
        let Commands::Changelog { command } = cli.command else {
            panic!("expected changelog");
        };
        assert_eq!(
            command,
            ChangelogCommands::Show {
                version: "v1.0.0".to_string(),
                include_header: true,
                path: Some(PathBuf::from("docs/HISTORY.md")),
                template: Some(PathBuf::from("notes.j2")),
            }
        );

        let cli_short = Cli::try_parse_from(["cutver", "changelog", "show", "0.3.1", "-H", "-p", "custom.md"]).unwrap();
        let Commands::Changelog { command: command_short } = cli_short.command else {
            panic!("expected changelog");
        };
        assert_eq!(
            command_short,
            ChangelogCommands::Show {
                version: "0.3.1".to_string(),
                include_header: true,
                path: Some(PathBuf::from("custom.md")),
                template: None,
            }
        );
    }

    #[test]
    fn config_override_global() {
        let cli = Cli::try_parse_from(["cutver", "-c", "other.toml", "doctor"]).unwrap();
        assert_eq!(cli.config, Some(PathBuf::from("other.toml")));
    }

    #[test]
    fn init_defaults() {
        let cli = Cli::try_parse_from(["cutver", "init"]).unwrap();
        let Commands::Init { update, force, path } = cli.command else {
            panic!("expected init");
        };
        assert!(!update);
        assert!(!force);
        assert!(path.is_none());
    }

    #[test]
    fn init_with_long_flags() {
        let cli = Cli::try_parse_from(["cutver", "init", "--update", "--force", "--path", "sub/dir"]).unwrap();
        let Commands::Init { update, force, path } = cli.command else {
            panic!("expected init");
        };
        assert!(update);
        assert!(force);
        assert_eq!(path, Some(PathBuf::from("sub/dir")));
    }

    #[test]
    fn bump_first_release_flags_and_defaults() {
        let cli = Cli::try_parse_from(["cutver", "bump", "--first-release"]).unwrap();
        let Commands::Bump {
            level,
            dry_run,
            skip_preflight,
            first_release,
        } = cli.command
        else {
            panic!("expected bump")
        };
        assert_eq!(level, BumpLevel::Auto);
        assert!(!dry_run);
        assert!(skip_preflight.is_empty());
        assert!(first_release);

        let cli2 = Cli::try_parse_from(["cutver", "bump", "auto", "--first-release"]).unwrap();
        let Commands::Bump {
            level: l2,
            first_release: fr2,
            ..
        } = cli2.command
        else {
            panic!("expected bump")
        };
        assert_eq!(l2, BumpLevel::Auto);
        assert!(fr2);

        let cli3 = Cli::try_parse_from(["cutver", "bump", "--fr"]).unwrap();
        let Commands::Bump { first_release: fr3, .. } = cli3.command else {
            panic!("expected bump")
        };
        assert!(fr3);
    }

    #[test]
    fn test_normalize_args() {
        let normalized = normalize_args(["cutver", "bump", "-fr"]);
        assert_eq!(normalized, vec!["cutver", "bump", "--first-release"]);

        let cli = Cli::try_parse_from(normalized).unwrap();
        let Commands::Bump { first_release, .. } = cli.command else {
            panic!("expected bump");
        };
        assert!(first_release);
    }

    #[test]
    fn init_with_short_flags() {
        let cli = Cli::try_parse_from(["cutver", "init", "-u", "-f", "-p", "sub/dir"]).unwrap();
        let Commands::Init { update, force, path } = cli.command else {
            panic!("expected init");
        };
        assert!(update);
        assert!(force);
        assert_eq!(path, Some(PathBuf::from("sub/dir")));
    }
}
