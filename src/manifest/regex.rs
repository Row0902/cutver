use super::{Error, ManifestEditor};
use regex::Regex;
use semver::Version;

#[derive(Debug)]
pub struct RegexEditor {
    pattern: String,
    replacement: String,
}

impl RegexEditor {
    pub fn new(pattern: String, replacement: String) -> Self {
        Self {
            pattern,
            replacement,
        }
    }
}

impl ManifestEditor for RegexEditor {
    fn read_version(&self, content: &str) -> Result<Version, Error> {
        let re = Regex::new(&self.pattern)?;
        let cap = re
            .captures(content)
            .ok_or_else(|| Error::NoMatch(self.pattern.clone()))?;
        let s = cap
            .get(1)
            .or_else(|| cap.get(0))
            .ok_or_else(|| Error::NoMatch(self.pattern.clone()))?
            .as_str();
        Version::parse(s).map_err(|e| Error::InvalidVersion(s.into(), e))
    }

    fn write_version(&self, content: &str, version: &Version) -> Result<String, Error> {
        let re = Regex::new(&self.pattern)?;
        if re.find_iter(content).count() == 0 {
            return Err(Error::NoMatch(self.pattern.clone()));
        }
        let replacement = self
            .replacement
            .replace("{{version}}", &version.to_string());
        Ok(re.replace_all(content, replacement.as_str()).into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"VERSION = "1.2.3"
# release marker
"#;

    #[test]
    fn reads_captured_version() {
        let v = RegexEditor::new(
            r#"VERSION = "([^"]+)""#.into(),
            r#"VERSION = "{{version}}""#.into(),
        )
        .read_version(FIXTURE)
        .unwrap();
        assert_eq!(v, Version::parse("1.2.3").unwrap());
    }

    #[test]
    fn writes_with_capture_group_and_version_token() {
        let out = RegexEditor::new(
            r#"VERSION = "([^"]+)""#.into(),
            r#"VERSION = "{{version}}""#.into(),
        )
        .write_version(FIXTURE, &Version::parse("1.3.0").unwrap())
        .unwrap();
        assert!(out.contains(r#"VERSION = "1.3.0""#), "{out}");
        assert!(out.contains("# release marker"));
    }

    #[test]
    fn writes_without_capture_group() {
        let out = RegexEditor::new(
            r"release/v\d+\.\d+\.\d+".into(),
            "release/v{{version}}".into(),
        )
        .write_version("url = release/v1.2.3", &Version::parse("2.0.0").unwrap())
        .unwrap();
        assert_eq!(out, "url = release/v2.0.0");
    }

    #[test]
    fn fails_when_no_match() {
        let e = RegexEditor::new(r"VERSION = (\d+)".into(), "x".into())
            .read_version("NOPE")
            .unwrap_err();
        assert!(matches!(e, Error::NoMatch(_)));
    }
}
