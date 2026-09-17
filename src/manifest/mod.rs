use semver::Version;
use thiserror::Error;

pub mod cargo_toml;
pub mod gradle;
pub mod json;
pub mod json_scan;
pub mod regex;

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to parse {kind}: {detail}")]
    Parse { kind: &'static str, detail: String },
    #[error("field '{0}' not found")]
    FieldNotFound(String),
    #[error("field '{0}' is not a string")]
    NotAString(String),
    #[error("invalid version string '{0}': {1}")]
    InvalidVersion(String, semver::Error),
    #[error("target '{0}' not found")]
    TargetNotFound(String),
    #[error("no match for pattern '{0}'")]
    NoMatch(String),
    #[error("regex error: {0}")]
    Regex(#[from] ::regex::Error),
    #[error("unsupported manifest kind '{0}'")]
    UnsupportedKind(String),
}

pub trait ManifestEditor: std::fmt::Debug + Send + Sync {
    fn read_version(&self, content: &str) -> Result<Version, Error>;
    fn write_version(&self, content: &str, version: &Version) -> Result<String, Error>;
}

use crate::config;

pub fn editor_for(entry: &config::Manifest) -> Result<Box<dyn ManifestEditor>, Error> {
    match entry.kind.as_str() {
        "json" => Ok(Box::new(json::JsonEditor::new(
            entry.field.clone().unwrap_or_default(),
        ))),
        "toml" | "cargo-package" => Ok(Box::new(cargo_toml::CargoEditor)),
        "gradle" => Ok(Box::new(gradle::GradleEditor::new(
            entry.version_name_field.clone().unwrap_or_default(),
            entry.version_code_field.clone().unwrap_or_default(),
        ))),
        "regex" => Ok(Box::new(regex::RegexEditor::new(
            entry.pattern.clone().unwrap_or_default(),
            entry.replacement.clone().unwrap_or_default(),
        ))),
        other => Err(Error::UnsupportedKind(other.into())),
    }
}
