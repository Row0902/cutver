use super::{Error, ManifestEditor};
use regex::Regex;
use semver::Version;

#[derive(Debug)]
pub struct GradleEditor {
    version_name_field: String,
    version_code_field: String,
}

impl GradleEditor {
    pub fn new(version_name_field: String, version_code_field: String) -> Self {
        Self {
            version_name_field,
            version_code_field,
        }
    }
}

impl ManifestEditor for GradleEditor {
    fn read_version(&self, content: &str) -> Result<Version, Error> {
        let re = Regex::new(&format!(r#"{}\s+"([^"]+)""#, regex::escape(&self.version_name_field)))?;
        let cap = re
            .captures(content)
            .ok_or_else(|| Error::TargetNotFound(self.version_name_field.clone()))?;
        let s = cap
            .get(1)
            .ok_or_else(|| Error::TargetNotFound(self.version_name_field.clone()))?
            .as_str();
        Version::parse(s).map_err(|e| Error::InvalidVersion(s.into(), e))
    }

    fn write_version(&self, content: &str, version: &Version) -> Result<String, Error> {
        let name_re = Regex::new(&format!(r#"{}\s+"[^"]+""#, regex::escape(&self.version_name_field)))?;
        if !name_re.is_match(content) {
            return Err(Error::TargetNotFound(self.version_name_field.clone()));
        }
        let out = name_re.replace_all(content, format!(r#"{} "{}""#, self.version_name_field, version));

        let code_re = Regex::new(&format!(r"{}\s+(\d+)", regex::escape(&self.version_code_field)))?;
        let cap = code_re
            .captures(&out)
            .ok_or_else(|| Error::TargetNotFound(self.version_code_field.clone()))?;
        let code: u64 = cap
            .get(1)
            .ok_or_else(|| Error::TargetNotFound(self.version_code_field.clone()))?
            .as_str()
            .parse()
            .map_err(|e| Error::Parse {
                kind: "gradle",
                detail: format!("{} is not a valid integer: {e}", self.version_code_field),
            })?;
        let result = code_re.replace_all(&out, format!(r#"{} {}"#, self.version_code_field, code + 1));

        Ok(result.into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KTS: &str = r#"plugins {
    id("com.android.application")
}
android {
    defaultConfig {
        versionName "1.2.3"
        versionCode 42
        applicationId = "com.example.app"
    }
}
"#;

    #[test]
    fn reads_version_name() {
        let v = GradleEditor::new("versionName".into(), "versionCode".into())
            .read_version(KTS)
            .unwrap();
        assert_eq!(v, Version::parse("1.2.3").unwrap());
    }

    #[test]
    fn writes_version_name_and_increments_code() {
        let out = GradleEditor::new("versionName".into(), "versionCode".into())
            .write_version(KTS, &Version::parse("1.3.0").unwrap())
            .unwrap();
        assert!(out.contains(r#"versionName "1.3.0""#), "{out}");
        assert!(out.contains("versionCode 43"), "{out}");
        assert!(out.contains("applicationId = \"com.example.app\""), "{out}");
    }

    #[test]
    fn fails_when_version_name_missing() {
        assert!(matches!(
            GradleEditor::new("versionName".into(), "versionCode".into()).read_version("android {}"),
            Err(Error::TargetNotFound(_))
        ));
    }

    #[test]
    fn fails_when_version_code_missing() {
        assert!(matches!(
            GradleEditor::new("versionName".into(), "versionCode".into())
                .write_version(r#"versionName "1.0.0""#, &Version::parse("1.0.1").unwrap()),
            Err(Error::TargetNotFound(_))
        ));
    }
}
