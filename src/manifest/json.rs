use super::{Error, ManifestEditor};
use semver::Version;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug)]
pub struct JsonEditor {
    field: String,
}

impl JsonEditor {
    pub fn new(field: String) -> Self {
        Self { field }
    }
}

fn navigate<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    let mut cur = value;
    for segment in path.split('.') {
        cur = cur.get(segment)?;
    }
    Some(cur)
}

fn navigate_mut<'a>(value: &'a mut Value, path: &str) -> Option<&'a mut Value> {
    let mut cur = value;
    for segment in path.split('.') {
        cur = cur.get_mut(segment)?;
    }
    Some(cur)
}

impl ManifestEditor for JsonEditor {
    fn read_version(&self, content: &str) -> Result<Version, Error> {
        let value: Value = serde_json::from_str(content).map_err(|e| Error::Parse {
            kind: "json",
            detail: e.to_string(),
        })?;
        let target = navigate(&value, &self.field)
            .ok_or_else(|| Error::FieldNotFound(self.field.clone()))?;
        let s = target.as_str().ok_or_else(|| {
            Error::InvalidVersion(
                format!("{} (not a string)", self.field),
                Version::parse("").unwrap_err(),
            )
        })?;
        Version::parse(s).map_err(|e| Error::InvalidVersion(s.into(), e))
    }

    fn write_version(&self, content: &str, version: &Version) -> Result<String, Error> {
        let mut value: Value = serde_json::from_str(content).map_err(|e| Error::Parse {
            kind: "json",
            detail: e.to_string(),
        })?;
        let target = navigate_mut(&mut value, &self.field)
            .ok_or_else(|| Error::FieldNotFound(self.field.clone()))?;
        *target = Value::String(version.to_string());
        let mut buf = Vec::new();
        let mut ser = serde_json::Serializer::with_formatter(
            &mut buf,
            serde_json::ser::PrettyFormatter::with_indent(b"  "),
        );
        value.serialize(&mut ser).map_err(|e| Error::Parse {
            kind: "json",
            detail: e.to_string(),
        })?;
        String::from_utf8(buf).map_err(|e| Error::Parse {
            kind: "json",
            detail: e.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIMPLE: &str = r#"{
  "name": "x",
  "version": "1.2.3"
}"#;

    const NESTED: &str = r#"{
  "project": {
    "version": "0.5.1",
    "name": "k"
  }
}"#;

    #[test]
    fn reads_top_level_version() {
        let v = JsonEditor::new("version".into())
            .read_version(SIMPLE)
            .unwrap();
        assert_eq!(v, Version::parse("1.2.3").unwrap());
    }

    #[test]
    fn reads_nested_version() {
        let v = JsonEditor::new("project.version".into())
            .read_version(NESTED)
            .unwrap();
        assert_eq!(v, Version::parse("0.5.1").unwrap());
    }

    #[test]
    fn writes_top_level_with_two_space_indent() {
        let out = JsonEditor::new("version".into())
            .write_version(SIMPLE, &Version::parse("1.2.4").unwrap())
            .unwrap();
        assert!(out.contains("\"version\": \"1.2.4\""));
        assert!(out.contains("\"name\": \"x\""));
        assert!(out.starts_with("{\n  \""));
    }

    #[test]
    fn writes_nested_version() {
        let out = JsonEditor::new("project.version".into())
            .write_version(NESTED, &Version::parse("0.6.0").unwrap())
            .unwrap();
        assert!(out.contains("\"version\": \"0.6.0\""));
        assert!(out.contains("\"name\": \"k\""));
    }

    #[test]
    fn fails_when_field_missing() {
        assert!(matches!(
            JsonEditor::new("missing".into()).read_version(SIMPLE),
            Err(Error::FieldNotFound(_))
        ));
    }

    #[test]
    fn fails_when_version_not_a_string() {
        let bad = r#"{"version": 123}"#;
        assert!(matches!(
            JsonEditor::new("version".into()).read_version(bad),
            Err(Error::InvalidVersion(_, _))
        ));
    }
}
