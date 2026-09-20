use super::{Error, ManifestEditor};
use semver::Version;
use toml_edit::{DocumentMut, Item, value};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PyprojectEditor {
    pub table: Option<String>,
}

impl PyprojectEditor {
    pub fn new(table: Option<String>) -> Self {
        Self { table }
    }
}

fn get_child<'a>(item: &'a Item, key: &str) -> Option<&'a Item> {
    item.as_table_like()?.get(key).filter(|i| !i.is_none())
}

fn get_child_mut<'a>(item: &'a mut Item, key: &str) -> Option<&'a mut Item> {
    item.as_table_like_mut()?.get_mut(key).filter(|i| !i.is_none())
}

impl ManifestEditor for PyprojectEditor {
    fn read_version(&self, content: &str) -> Result<Version, Error> {
        let doc = content.parse::<DocumentMut>().map_err(|e| Error::Parse {
            kind: "pyproject-toml",
            detail: e.to_string(),
        })?;

        let version_str = match &self.table {
            Some(t) => {
                let mut cur = doc.as_item();
                for seg in t.split('.') {
                    cur = get_child(cur, seg).ok_or_else(|| Error::FieldNotFound(format!("{t}.version")))?;
                }
                let version_item =
                    get_child(cur, "version").ok_or_else(|| Error::FieldNotFound(format!("{t}.version")))?;
                version_item
                    .as_str()
                    .ok_or_else(|| Error::FieldNotFound(format!("{t}.version")))?
            }
            None => {
                if let Some(v) = doc
                    .get("project")
                    .and_then(|p| p.get("version"))
                    .and_then(|v| v.as_str())
                {
                    v
                } else if let Some(v) = doc
                    .get("tool")
                    .and_then(|t| t.get("poetry"))
                    .and_then(|p| p.get("version"))
                    .and_then(|v| v.as_str())
                {
                    v
                } else {
                    return Err(Error::FieldNotFound("project.version or tool.poetry.version".into()));
                }
            }
        };

        Version::parse(version_str).map_err(|e| Error::InvalidVersion(version_str.into(), e))
    }

    fn write_version(&self, content: &str, version: &Version) -> Result<String, Error> {
        let mut doc = content.parse::<DocumentMut>().map_err(|e| Error::Parse {
            kind: "pyproject-toml",
            detail: e.to_string(),
        })?;

        let (table_path, error_field): (Vec<&str>, String) = match &self.table {
            Some(t) => (t.split('.').collect(), format!("{t}.version")),
            None => {
                if doc
                    .get("project")
                    .and_then(|p| p.get("version"))
                    .and_then(|v| v.as_str())
                    .is_some()
                {
                    (vec!["project"], "project.version".to_string())
                } else if doc
                    .get("tool")
                    .and_then(|t| t.get("poetry"))
                    .and_then(|p| p.get("version"))
                    .and_then(|v| v.as_str())
                    .is_some()
                {
                    (vec!["tool", "poetry"], "tool.poetry.version".to_string())
                } else {
                    return Err(Error::FieldNotFound("project.version or tool.poetry.version".into()));
                }
            }
        };

        let mut cur = doc.as_item_mut();
        for seg in table_path {
            cur = get_child_mut(cur, seg).ok_or_else(|| Error::FieldNotFound(error_field.clone()))?;
        }
        let version_item = get_child_mut(cur, "version").ok_or(Error::FieldNotFound(error_field))?;

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

    const PEP621_FIXTURE: &str = r#"[project]
name = "my-app"
version = "0.1.0"
description = "PEP 621 metadata"
"#;

    const PEP621_WITH_COMMENTS: &str = r#"[project]
name = "my-app"
version = "0.1.0" # current release
description = "PEP 621 metadata"

[dependencies]
requests = ">=2.0"
"#;

    const POETRY_FIXTURE: &str = r#"[tool.poetry]
name = "poetry-app"
version = "1.2.3"
description = "Poetry project"

[tool.poetry.dependencies]
python = "^3.11"
"#;

    const POETRY_WITH_COMMENTS: &str = r#"[tool.poetry]
name = "poetry-app"
version = "1.2.3" # poetry semver
description = "Poetry project"
"#;

    const CUSTOM_FIXTURE: &str = r#"[tool.flit.metadata]
module = "flit_pkg"
version = "0.5.0" # flit version
author = "Dev"
"#;

    #[test]
    fn reads_pep621_project_version() {
        let editor = PyprojectEditor { table: None };
        let v = editor.read_version(PEP621_FIXTURE).unwrap();
        assert_eq!(v, Version::parse("0.1.0").unwrap());
    }

    #[test]
    fn writes_pep621_project_version_preserving_comments() {
        let editor = PyprojectEditor { table: None };
        let out = editor
            .write_version(PEP621_WITH_COMMENTS, &Version::parse("0.2.0").unwrap())
            .unwrap();
        assert!(out.contains(r#"version = "0.2.0" # current release"#), "{out}");
        assert!(out.contains("name = \"my-app\""));
        assert!(out.contains("[dependencies]"));
    }

    #[test]
    fn reads_poetry_version() {
        let editor = PyprojectEditor { table: None };
        let v = editor.read_version(POETRY_FIXTURE).unwrap();
        assert_eq!(v, Version::parse("1.2.3").unwrap());
    }

    #[test]
    fn writes_poetry_version_preserving_comments() {
        let editor = PyprojectEditor { table: None };
        let out = editor
            .write_version(POETRY_WITH_COMMENTS, &Version::parse("1.3.0").unwrap())
            .unwrap();
        assert!(out.contains(r#"version = "1.3.0" # poetry semver"#), "{out}");
        assert!(out.contains("name = \"poetry-app\""));
    }

    #[test]
    fn reads_and_writes_custom_table() {
        let editor = PyprojectEditor {
            table: Some("tool.flit.metadata".into()),
        };
        let v = editor.read_version(CUSTOM_FIXTURE).unwrap();
        assert_eq!(v, Version::parse("0.5.0").unwrap());

        let out = editor
            .write_version(CUSTOM_FIXTURE, &Version::parse("0.6.0").unwrap())
            .unwrap();
        assert!(out.contains(r#"version = "0.6.0" # flit version"#), "{out}");
        assert!(out.contains("module = \"flit_pkg\""));
    }

    #[test]
    fn fails_when_version_missing() {
        let editor = PyprojectEditor { table: None };
        assert!(matches!(
            editor.read_version(""),
            Err(Error::FieldNotFound(msg)) if msg == "project.version or tool.poetry.version"
        ));
        assert!(matches!(
            editor.read_version("[project]\nname = \"foo\""),
            Err(Error::FieldNotFound(msg)) if msg == "project.version or tool.poetry.version"
        ));
        assert!(matches!(
            editor.read_version("[tool.other]\nname = \"foo\""),
            Err(Error::FieldNotFound(msg)) if msg == "project.version or tool.poetry.version"
        ));
        assert!(matches!(
            editor.write_version("[project]\nname = \"foo\"", &Version::parse("1.0.0").unwrap()),
            Err(Error::FieldNotFound(msg)) if msg == "project.version or tool.poetry.version"
        ));

        let custom_editor = PyprojectEditor {
            table: Some("tool.flit.metadata".into()),
        };
        assert!(matches!(
            custom_editor.read_version("[project]\nversion = \"1.0.0\""),
            Err(Error::FieldNotFound(msg)) if msg == "tool.flit.metadata.version"
        ));
        assert!(matches!(
            custom_editor.write_version("[project]\nversion = \"1.0.0\"", &Version::parse("1.0.0").unwrap()),
            Err(Error::FieldNotFound(msg)) if msg == "tool.flit.metadata.version"
        ));
    }

    #[test]
    fn fails_when_version_invalid() {
        let editor = PyprojectEditor { table: None };
        assert!(matches!(
            editor.read_version("[project]\nversion = \"not-a-semver\""),
            Err(Error::InvalidVersion(v, _)) if v == "not-a-semver"
        ));
        assert!(matches!(
            editor.read_version("[tool.poetry]\nversion = \"not-a-semver\""),
            Err(Error::InvalidVersion(v, _)) if v == "not-a-semver"
        ));

        let custom_editor = PyprojectEditor {
            table: Some("tool.custom".into()),
        };
        assert!(matches!(
            custom_editor.read_version("[tool.custom]\nversion = \"invalid\""),
            Err(Error::InvalidVersion(v, _)) if v == "invalid"
        ));
    }
}
