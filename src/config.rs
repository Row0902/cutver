use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::Path;
use serde::Deserialize;
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
    #[error("manifest '{0}' has unknown kind '{1}'")]
    UnknownKind(String, String),
    #[error("manifest '{0}' of kind '{1}' is missing required field '{2}'")]
    MissingField(String, String, String),
    #[error("preflight command '{0}' must be a string")]
    PreflightNotString(String),
}
#[derive(Debug, Deserialize)]
pub struct Config {
    pub version: VersionSection,
    #[serde(default)]
    pub manifest: Vec<Manifest>,
    #[serde(skip)]
    pub preflight: Vec<(String, String)>,
    #[serde(default)]
    pub changelog: Changelog,
    #[serde(default)]
    pub git: Git,
}
#[derive(Debug, Deserialize)]
pub struct VersionSection {
    pub current_source: String,
}
#[derive(Debug, Deserialize)]
pub struct Manifest {
    pub path: String,
    pub kind: String,
    pub field: Option<String>,
    pub version_name_field: Option<String>,
    pub version_code_field: Option<String>,
    pub pattern: Option<String>,
    pub replacement: Option<String>,
}
#[derive(Debug, Deserialize)]
pub struct Changelog {
    pub path: Option<String>,
    #[serde(default = "default_format")]
    pub format: String,
    #[serde(default = "default_entry_template")]
    pub entry_template: String,
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
impl Default for Changelog {
    fn default() -> Self { Self { path: None, format: default_format(), entry_template: default_entry_template() } }
}
impl Default for Git {
    fn default() -> Self { Self { tag_prefix: default_tag_prefix(), commit_message: default_commit_message(), require_clean_tree: default_require_clean_tree(), require_branch: None } }
}

fn default_format() -> String { "keep-a-changelog".into() }
fn default_entry_template() -> String { "Maintenance and updates.".into() }
fn default_tag_prefix() -> String { "v".into() }
fn default_commit_message() -> String { "chore(release): v{version}".into() }
fn default_require_clean_tree() -> bool { true }

pub fn load(path: impl AsRef<Path>) -> Result<Config, ConfigError> {
    let text = fs::read_to_string(path)?;
    let mut config: Config = toml::from_str(&text)?;
    config.preflight = parse_preflight(&text)?;
    validate(&config)?;
    Ok(config)
}

fn parse_preflight(text: &str) -> Result<Vec<(String, String)>, ConfigError> {
    let doc = text.parse::<toml_edit::DocumentMut>()?;
    let mut cmds = Vec::new();
    if let Some(table) = doc.get("preflight").and_then(|v| v.as_table()) {
        for (k, v) in table.iter() {
            cmds.push((k.into(), v.as_str().ok_or_else(|| ConfigError::PreflightNotString(k.into()))?.into()));
        }
    }
    Ok(cmds)
}

fn validate(config: &Config) -> Result<(), ConfigError> {
    let mut seen = HashSet::new();
    for m in &config.manifest {
        if !seen.insert(&m.path) { return Err(ConfigError::DuplicateManifestPath(m.path.clone())); }
    }
    if !config.manifest.iter().any(|m| m.path == config.version.current_source) {
        return Err(ConfigError::CurrentSourceNotFound(config.version.current_source.clone()));
    }
    for m in &config.manifest {
        match m.kind.as_str() {
            "json" => require(m.field.as_ref(), m, "field")?,
            "gradle" => {
                require(m.version_name_field.as_ref(), m, "version_name_field")?;
                require(m.version_code_field.as_ref(), m, "version_code_field")?;
            }
            "regex" => {
                require(m.pattern.as_ref(), m, "pattern")?;
                require(m.replacement.as_ref(), m, "replacement")?;
            }
            "toml" | "cargo-package" => {}
            _ => return Err(ConfigError::UnknownKind(m.path.clone(), m.kind.clone())),
        }
    }
    Ok(())
}

fn require<T>(opt: Option<&T>, m: &Manifest, field: &str) -> Result<(), ConfigError> {
    if opt.is_none() { Err(ConfigError::MissingField(m.path.clone(), m.kind.clone(), field.into())) } else { Ok(()) }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn load_str(s: &str) -> Result<Config, ConfigError> {
        let mut config: Config = toml::from_str(s).map_err(ConfigError::Parse)?;
        config.preflight = parse_preflight(s)?;
        validate(&config)?;
        Ok(config)
    }

    fn manifest(kind: &str, extra: &str) -> String {
        format!("[version]\ncurrent_source = \"a\"\n\n[[manifest]]\npath = \"a\"\nkind = \"{kind}\"\n{extra}")
    }

    #[test]
    fn valid_fixture_parses() {
        let cfg = load_str(r#"
[version]
current_source = "package.json"
[[manifest]]
path = "package.json"
kind = "json"
field = "version"
[[manifest]]
path = "Cargo.toml"
kind = "cargo-package"
[preflight]
tests = "cargo test"
[changelog]
path = "CHANGELOG.md"
"#).unwrap();
        assert_eq!(cfg.version.current_source, "package.json");
        assert_eq!(cfg.manifest.len(), 2);
        assert_eq!(cfg.preflight.len(), 1);
        assert_eq!(cfg.git.tag_prefix, "v");
    }

    #[test]
    fn defaults_applied() {
        let cfg = load_str(&manifest("cargo-package", "")).unwrap();
        assert_eq!(cfg.changelog.format, "keep-a-changelog");
        assert_eq!(cfg.changelog.entry_template, "Maintenance and updates.");
        assert_eq!(cfg.git.tag_prefix, "v");
        assert_eq!(cfg.git.commit_message, "chore(release): v{version}");
        assert!(cfg.git.require_clean_tree);
        assert!(cfg.git.require_branch.is_none());
    }

    #[test]
    fn preflight_preserves_declaration_order() {
        let cfg = load_str(&(manifest("cargo-package", "") + "\n[preflight]\nz = \"z\"\na = \"a\"\nm = \"m\"")).unwrap();
        assert_eq!(cfg.preflight.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(), vec!["z", "a", "m"]);
    }

    #[test]
    fn validation_failures() {
        let dup = "[version]\ncurrent_source = \"a\"\n\n[[manifest]]\npath = \"a\"\nkind = \"cargo-package\"\n\n[[manifest]]\npath = \"a\"\nkind = \"json\"\nfield = \"version\"";
        let missing = "[version]\ncurrent_source = \"missing\"\n\n[[manifest]]\npath = \"a\"\nkind = \"cargo-package\"";
        assert!(matches!(load_str(dup), Err(ConfigError::DuplicateManifestPath(_))));
        assert!(matches!(load_str(missing), Err(ConfigError::CurrentSourceNotFound(_))));
        assert!(matches!(load_str(&manifest("json", "")), Err(ConfigError::MissingField(_, _, _))));
        assert!(matches!(load_str(&manifest("gradle", "")), Err(ConfigError::MissingField(_, _, _))));
        assert!(matches!(load_str(&manifest("regex", "")), Err(ConfigError::MissingField(_, _, _))));
        assert!(matches!(load_str(&manifest("unknown", "")), Err(ConfigError::UnknownKind(_, _))));
        assert!(matches!(load_str(&(manifest("cargo-package", "") + "\n[preflight]\ntests = 123")), Err(ConfigError::PreflightNotString(_))));
    }
}
