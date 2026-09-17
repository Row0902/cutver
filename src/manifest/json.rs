use super::{Error, ManifestEditor, json_scan};
use semver::Version;
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

impl ManifestEditor for JsonEditor {
    fn read_version(&self, content: &str) -> Result<Version, Error> {
        let value: Value = serde_json::from_str(content).map_err(|e| Error::Parse {
            kind: "json",
            detail: e.to_string(),
        })?;
        let target = navigate(&value, &self.field).ok_or_else(|| Error::FieldNotFound(self.field.clone()))?;
        let s = target.as_str().ok_or_else(|| Error::NotAString(self.field.clone()))?;
        Version::parse(s).map_err(|e| Error::InvalidVersion(s.into(), e))
    }

    fn write_version(&self, content: &str, version: &Version) -> Result<String, Error> {
        let span = json_scan::find_string_value(content, &self.field)?;
        let mut out = content.to_string();
        out.replace_range(span, &format!("\"{}\"", version));
        Ok(out)
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

    const UNSORTED: &str = "{\n  \"z\": 1,\n  \"a\": 2,\n  \"version\": \"1.0.0\"\n}\n";

    const INDENTED: &str = "{\n    \"version\": \"1.0.0\"\n}\n";

    #[test]
    fn reads_top_level_version() {
        assert_eq!(
            JsonEditor::new("version".into()).read_version(SIMPLE).unwrap(),
            Version::parse("1.2.3").unwrap()
        );
    }

    #[test]
    fn reads_nested_version() {
        assert_eq!(
            JsonEditor::new("project.version".into()).read_version(NESTED).unwrap(),
            Version::parse("0.5.1").unwrap()
        );
    }

    #[test]
    fn writes_preserves_key_order() {
        let out = JsonEditor::new("version".into())
            .write_version(UNSORTED, &Version::parse("1.0.1").unwrap())
            .unwrap();
        let z = out.find("\"z\"").unwrap();
        let a = out.find("\"a\"").unwrap();
        let v = out.find("\"version\"").unwrap();
        assert!(z < a && a < v);
    }

    #[test]
    fn writes_preserves_trailing_newline() {
        let out = JsonEditor::new("version".into())
            .write_version(UNSORTED, &Version::parse("1.0.1").unwrap())
            .unwrap();
        assert_eq!(out.chars().last(), Some('\n'));
    }

    #[test]
    fn writes_preserves_indentation() {
        let out = JsonEditor::new("version".into())
            .write_version(INDENTED, &Version::parse("2.0.0").unwrap())
            .unwrap();
        assert!(out.contains("    \"version\": \"2.0.0\""));
    }

    #[test]
    fn writes_replaces_only_value_substring() {
        let before = UNSORTED;
        let after = JsonEditor::new("version".into())
            .write_version(before, &Version::parse("9.8.7").unwrap())
            .unwrap();
        let start = before.find("\"1.0.0\"").unwrap();
        let end = start + "\"1.0.0\"".len();
        assert_eq!(&before[..start], &after[..start]);
        assert_eq!(&before[end..], &after[end..]);
        assert_eq!(&after[start..end], "\"9.8.7\"");
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
        assert!(matches!(
            JsonEditor::new("version".into()).read_version(r#"{"version": 123}"#),
            Err(Error::NotAString(_))
        ));
    }

    #[test]
    fn write_fails_when_field_missing() {
        assert!(matches!(
            JsonEditor::new("missing".into()).write_version(SIMPLE, &Version::parse("1.0.0").unwrap()),
            Err(Error::FieldNotFound(_))
        ));
    }

    #[test]
    fn write_fails_when_version_not_a_string() {
        assert!(matches!(
            JsonEditor::new("version".into()).write_version(r#"{"version": 123}"#, &Version::parse("1.0.0").unwrap()),
            Err(Error::NotAString(_))
        ));
    }
}
