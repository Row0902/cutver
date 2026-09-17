use super::{Error, ManifestEditor};
use semver::Version;
use toml_edit::{DocumentMut, value};

#[derive(Debug)]
pub struct CargoEditor;

impl ManifestEditor for CargoEditor {
    fn read_version(&self, content: &str) -> Result<Version, Error> {
        let doc = content.parse::<DocumentMut>().map_err(|e| Error::Parse {
            kind: "cargo-toml",
            detail: e.to_string(),
        })?;
        let v = doc
            .get("package")
            .and_then(|p| p.get("version"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::FieldNotFound("package.version".into()))?;
        Version::parse(v).map_err(|e| Error::InvalidVersion(v.into(), e))
    }

    fn write_version(&self, content: &str, version: &Version) -> Result<String, Error> {
        let mut doc = content.parse::<DocumentMut>().map_err(|e| Error::Parse {
            kind: "cargo-toml",
            detail: e.to_string(),
        })?;
        let package = doc
            .get_mut("package")
            .ok_or_else(|| Error::FieldNotFound("package".into()))?;
        let version_item = package
            .get_mut("version")
            .ok_or_else(|| Error::FieldNotFound("package.version".into()))?;
        let old_decor = version_item.as_value().map(|v| v.decor().clone());
        *version_item = value(version.to_string());
        if let (Some(v), Some(d)) = (version_item.as_value_mut(), old_decor) {
            *v.decor_mut() = d;
        }
        Ok(doc.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"[package]
name = "cutver"
version = "0.1.0" # current
edition = "2024"

[dependencies]
clap = "4"
"#;

    #[test]
    fn reads_package_version() {
        let v = CargoEditor.read_version(FIXTURE).unwrap();
        assert_eq!(v, Version::parse("0.1.0").unwrap());
    }

    #[test]
    fn writes_package_version_preserving_comments() {
        let out = CargoEditor
            .write_version(FIXTURE, &Version::parse("0.2.0").unwrap())
            .unwrap();
        assert!(out.contains(r#"version = "0.2.0" # current"#), "{out}");
        assert!(out.contains("name = \"cutver\""));
        assert!(out.contains("edition = \"2024\""));
        assert!(out.contains("[dependencies]"));
    }

    #[test]
    fn fails_when_package_missing() {
        assert!(matches!(
            CargoEditor.read_version("[foo]"),
            Err(Error::FieldNotFound(_))
        ));
    }

    #[test]
    fn fails_when_version_missing() {
        assert!(matches!(
            CargoEditor.read_version("[package]\nname = \"x\""),
            Err(Error::FieldNotFound(_))
        ));
    }
}
