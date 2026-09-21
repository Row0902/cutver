use serde::Deserialize;
use std::io;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config file: {0}")]
    Read(#[from] io::Error),
    #[error("failed to parse config file: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("failed to parse config document: {0}")]
    DocumentParse(#[from] toml_edit::TomlError),
    #[error("duplicate manifest path: {0}")]
    DuplicateManifestPath(String),
    #[error("current_source '{0}' is not declared as a manifest path")]
    CurrentSourceNotFound(String),
    #[error("preflight step '{0}' must be a string or inline table")]
    PreflightNotString(String),
    #[error("preflight step '{0}' is missing a string 'command' field")]
    PreflightMissingCommand(String),
    #[error("preflight timeout for '{0}' must be a positive integer")]
    PreflightInvalidTimeout(String),
    #[error(
        "no cutver.toml or release.toml found in '{0}' or any parent directory.\n  Get started by running:\n    cutver init"
    )]
    NotFound(String),
    #[error("no manifests declared: at least one [[manifest]] entry is required")]
    NoManifestsDeclared,
    #[error("multiple manifests are marked with primary = true: only one manifest may be primary")]
    MultiplePrimaryManifests,
    #[error("conflicting primary manifest: [version] specifies '{0}' but '{1}' is marked as primary")]
    ConflictingPrimaryManifest(String, String),
}

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(skip)]
    pub root_dir: PathBuf,
    #[serde(default)]
    pub version: VersionSection,
    #[serde(default)]
    pub manifest: Vec<Manifest>,
    #[serde(skip)]
    pub preflight: PreflightSteps,
    #[serde(skip)]
    pub preflight_default_timeout: Option<u64>,
    #[serde(default)]
    pub changelog: Changelog,
    #[serde(default)]
    pub git: Git,
    #[serde(default)]
    pub hooks: Hooks,
    #[serde(default)]
    pub publish: Publish,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub struct Hooks {
    pub pre_bump: Option<String>,
    pub post_bump: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub struct Publish {
    #[serde(default)]
    pub push: bool,
    #[serde(default)]
    pub commands: Vec<String>,
    pub default_timeout: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct VersionSection {
    #[serde(alias = "source", default)]
    pub current_source: String,
    #[serde(default = "default_strategy")]
    pub strategy: String,
}

pub(crate) fn default_strategy() -> String {
    "manual".into()
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ManifestKind {
    Json {
        field: String,
    },
    #[serde(alias = "toml")]
    CargoPackage,
    Gradle {
        version_name_field: String,
        version_code_field: String,
    },
    Regex {
        pattern: String,
        replacement: String,
    },
    #[serde(alias = "pyproject-toml")]
    Pyproject {
        #[serde(default)]
        table: Option<String>,
    },
}

#[derive(Debug, Clone, Deserialize)]
pub struct Manifest {
    pub path: String,
    #[serde(default)]
    pub primary: bool,
    #[serde(flatten)]
    pub kind: ManifestKind,
}

#[derive(Debug, Clone)]
pub struct PreflightCommand {
    pub command: String,
    pub timeout: Option<u64>,
}

/// Ordered `[preflight]` steps as declared in `release.toml`.
pub type PreflightSteps = Vec<(String, PreflightCommand)>;

/// Global `[preflight] default_timeout`.
pub type PreflightDefault = Option<u64>;

#[derive(Debug, Clone, Deserialize)]
pub struct Changelog {
    pub path: Option<String>,
    #[serde(default = "default_format")]
    pub format: String,
    #[serde(default = "default_entry_template")]
    pub entry_template: String,
    #[serde(default = "default_changelog_mode")]
    pub mode: String,
    #[serde(default = "default_include_scopes")]
    pub include_scopes: bool,
    #[serde(default = "default_fallback_entry")]
    pub fallback_entry: String,
}

impl Default for Changelog {
    fn default() -> Self {
        Self {
            path: None,
            format: default_format(),
            entry_template: default_entry_template(),
            mode: default_changelog_mode(),
            include_scopes: default_include_scopes(),
            fallback_entry: default_fallback_entry(),
        }
    }
}

fn default_format() -> String {
    "keep-a-changelog".into()
}

fn default_entry_template() -> String {
    "Maintenance and updates.".into()
}

fn default_changelog_mode() -> String {
    "conventional".into()
}

fn default_include_scopes() -> bool {
    true
}

fn default_fallback_entry() -> String {
    "Maintenance and updates.".into()
}

#[derive(Debug, Deserialize)]
pub struct Git {
    #[serde(default = "default_tag_prefix")]
    pub tag_prefix: String,
    #[serde(default = "default_commit_message")]
    pub commit_message: String,
    #[serde(default = "default_require_clean_tree")]
    pub require_clean_tree: bool,
    pub require_branch: Option<String>,
}

impl Default for Git {
    fn default() -> Self {
        Self {
            tag_prefix: default_tag_prefix(),
            commit_message: default_commit_message(),
            require_clean_tree: default_require_clean_tree(),
            require_branch: None,
        }
    }
}

fn default_tag_prefix() -> String {
    "v".into()
}

fn default_commit_message() -> String {
    "chore(release): v{version}".into()
}

fn default_require_clean_tree() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(kind: &str, extra: &str) -> String {
        format!("[version]\ncurrent_source = \"a\"\n\n[[manifest]]\npath = \"a\"\nkind = \"{kind}\"\n{extra}")
    }

    fn load_str(s: &str) -> Result<Config, ConfigError> {
        let mut c: Config = toml::from_str(s).map_err(ConfigError::Parse)?;
        let (p, d) = super::super::preflight::parse_preflight(s)?;
        c.preflight = p;
        c.preflight_default_timeout = d;
        c.root_dir = std::env::current_dir().unwrap();
        super::super::validation::deduce_current_source(&mut c)?;
        super::super::validation::validate(&c)?;
        Ok(c)
    }

    #[test]
    fn changelog_defaults_and_custom_config() {
        let default_cl = Changelog::default();
        assert_eq!(default_cl.format, "keep-a-changelog");
        assert_eq!(default_cl.entry_template, "Maintenance and updates.");
        assert_eq!(default_cl.mode, "conventional");
        assert!(default_cl.include_scopes);
        assert_eq!(default_cl.fallback_entry, "Maintenance and updates.");

        let toml = r#"
[[manifest]]
path = "Cargo.toml"
kind = "cargo-package"

[changelog]
path = "CHANGELOG.md"
mode = "template"
include_scopes = false
fallback_entry = "Custom fallback notes."
"#;
        let c = load_str(toml).unwrap();
        assert_eq!(c.changelog.mode, "template");
        assert!(!c.changelog.include_scopes);
        assert_eq!(c.changelog.fallback_entry, "Custom fallback notes.");
    }

    #[test]
    fn hooks_and_publish_parsing_and_defaults() {
        let toml = r#"
[[manifest]]
path = "Cargo.toml"
kind = "cargo-package"

[hooks]
pre_bump = "echo pre"
post_bump = "cargo check"

[publish]
push = true
commands = ["cargo publish", "gh release create v{version}"]
default_timeout = 60
"#;
        let c = load_str(toml).unwrap();
        assert_eq!(c.hooks.pre_bump.as_deref(), Some("echo pre"));
        assert_eq!(c.hooks.post_bump.as_deref(), Some("cargo check"));
        assert!(c.publish.push);
        assert_eq!(
            c.publish.commands,
            vec!["cargo publish", "gh release create v{version}"]
        );
        assert_eq!(c.publish.default_timeout, Some(60));

        let default_cfg = load_str(&manifest("cargo-package", "")).unwrap();
        assert_eq!(default_cfg.hooks.pre_bump, None);
        assert_eq!(default_cfg.hooks.post_bump, None);
        assert!(!default_cfg.publish.push);
        assert!(default_cfg.publish.commands.is_empty());
        assert_eq!(default_cfg.publish.default_timeout, None);
    }

    #[test]
    fn pyproject_manifest_config_parsing() {
        let cfg = load_str(&manifest("pyproject", "")).unwrap();
        assert_eq!(cfg.manifest[0].kind, ManifestKind::Pyproject { table: None });
    }

    #[test]
    fn pyproject_manifest_with_table() {
        let cfg = load_str(&manifest("pyproject", "table = \"tool.poetry\"\n")).unwrap();
        assert_eq!(
            cfg.manifest[0].kind,
            ManifestKind::Pyproject {
                table: Some("tool.poetry".to_string()),
            }
        );
    }

    #[test]
    fn pyproject_manifest_alias() {
        let cfg = load_str(&manifest("pyproject-toml", "")).unwrap();
        assert_eq!(cfg.manifest[0].kind, ManifestKind::Pyproject { table: None });
    }
}
