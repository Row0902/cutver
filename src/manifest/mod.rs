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
}

pub trait ManifestEditor: std::fmt::Debug + Send + Sync {
    fn read_version(&self, content: &str) -> Result<Version, Error>;
    fn write_version(&self, content: &str, version: &Version) -> Result<String, Error>;
}

use crate::config;

pub fn editor_for(entry: &config::Manifest) -> Result<Box<dyn ManifestEditor>, Error> {
    match &entry.kind {
        config::ManifestKind::Json { field } => Ok(Box::new(json::JsonEditor::new(field.clone()))),
        config::ManifestKind::CargoPackage => Ok(Box::new(cargo_toml::CargoEditor)),
        config::ManifestKind::Gradle {
            version_name_field,
            version_code_field,
        } => Ok(Box::new(gradle::GradleEditor::try_new(
            version_name_field.clone(),
            version_code_field.clone(),
        )?)),
        config::ManifestKind::Regex { pattern, replacement } => {
            Ok(Box::new(regex::RegexEditor::new(pattern.clone(), replacement.clone())))
        }
    }
}
