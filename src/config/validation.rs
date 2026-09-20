use std::collections::HashSet;

use super::types::{Config, ConfigError, Manifest, default_strategy};

pub(crate) fn deduce_current_source(config: &mut Config) -> Result<(), ConfigError> {
    if config.manifest.is_empty() {
        return Err(ConfigError::NoManifestsDeclared);
    }
    let candidates: Vec<&Manifest> = config.manifest.iter().filter(|m| m.primary).collect();
    if candidates.len() > 1 {
        return Err(ConfigError::MultiplePrimaryManifests);
    }
    let primary_path = candidates.first().map(|m| m.path.as_str());
    if config.version.current_source.is_empty() {
        if let Some(p) = primary_path {
            config.version.current_source = p.to_string();
        } else {
            config.version.current_source = config.manifest[0].path.clone();
        }
    } else if let Some(p) = primary_path
        && config.version.current_source != p
    {
        return Err(ConfigError::ConflictingPrimaryManifest(
            config.version.current_source.clone(),
            p.to_string(),
        ));
    }
    if config.version.strategy.is_empty() {
        config.version.strategy = default_strategy();
    }
    Ok(())
}

pub(crate) fn validate(config: &Config) -> Result<(), ConfigError> {
    let mut seen = HashSet::new();
    for m in &config.manifest {
        if !seen.insert(&m.path) {
            return Err(ConfigError::DuplicateManifestPath(m.path.clone()));
        }
    }
    if !config.manifest.iter().any(|m| m.path == config.version.current_source) {
        return Err(ConfigError::CurrentSourceNotFound(
            config.version.current_source.clone(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_str(s: &str) -> Result<Config, ConfigError> {
        let mut c: Config = toml::from_str(s).map_err(ConfigError::Parse)?;
        let (p, d) = super::super::preflight::parse_preflight(s)?;
        c.preflight = p;
        c.preflight_default_timeout = d;
        c.root_dir = std::env::current_dir().unwrap();
        deduce_current_source(&mut c)?;
        validate(&c)?;
        Ok(c)
    }

    fn manifest(kind: &str, extra: &str) -> String {
        format!("[version]\ncurrent_source = \"a\"\n\n[[manifest]]\npath = \"a\"\nkind = \"{kind}\"\n{extra}")
    }

    #[test]
    fn validation_failures() {
        let dup = "[version]\ncurrent_source = \"a\"\n\n[[manifest]]\npath = \"a\"\nkind = \"cargo-package\"\n\n[[manifest]]\npath = \"a\"\nkind = \"json\"\nfield = \"version\"";
        let missing = "[version]\ncurrent_source = \"missing\"\n\n[[manifest]]\npath = \"a\"\nkind = \"cargo-package\"";
        assert!(matches!(load_str(dup), Err(ConfigError::DuplicateManifestPath(_))));
        assert!(matches!(load_str(missing), Err(ConfigError::CurrentSourceNotFound(_))));
        assert!(matches!(load_str(&manifest("json", "")), Err(ConfigError::Parse(_))));
        assert!(matches!(load_str(&manifest("gradle", "")), Err(ConfigError::Parse(_))));
        assert!(matches!(load_str(&manifest("regex", "")), Err(ConfigError::Parse(_))));
        assert!(matches!(load_str(&manifest("unknown", "")), Err(ConfigError::Parse(_))));
        assert!(matches!(
            load_str(&(manifest("cargo-package", "") + "\n[preflight]\ntests = 123")),
            Err(ConfigError::PreflightNotString(_))
        ));
    }

    #[test]
    fn omit_version_uses_first_manifest_as_source() {
        let toml = r#"
[[manifest]]
path = "Cargo.toml"
kind = "cargo-package"

[[manifest]]
path = "package.json"
kind = "json"
field = "version"
"#;
        let cfg = load_str(toml).unwrap();
        assert_eq!(cfg.version.current_source, "Cargo.toml");
        assert_eq!(cfg.version.strategy, "manual");
    }

    #[test]
    fn version_source_alias_works() {
        let toml = r#"
[version]
source = "package.json"

[[manifest]]
path = "Cargo.toml"
kind = "cargo-package"

[[manifest]]
path = "package.json"
kind = "json"
field = "version"
"#;
        let cfg = load_str(toml).unwrap();
        assert_eq!(cfg.version.current_source, "package.json");
    }

    #[test]
    fn primary_manifest_on_non_first_manifest_uses_it_as_source() {
        let toml = r#"
[[manifest]]
path = "package.json"
kind = "json"
field = "version"

[[manifest]]
path = "Cargo.toml"
kind = "cargo-package"
primary = true
"#;
        let cfg = load_str(toml).unwrap();
        assert_eq!(cfg.version.current_source, "Cargo.toml");
    }

    #[test]
    fn multiple_primary_manifests_errors() {
        let toml = r#"
[[manifest]]
path = "Cargo.toml"
kind = "cargo-package"
primary = true

[[manifest]]
path = "package.json"
kind = "json"
field = "version"
primary = true
"#;
        let err = load_str(toml).unwrap_err();
        assert!(matches!(err, ConfigError::MultiplePrimaryManifests));
    }

    #[test]
    fn conflicting_current_source_and_primary_manifest_errors() {
        let toml = r#"
[version]
current_source = "package.json"

[[manifest]]
path = "package.json"
kind = "json"
field = "version"

[[manifest]]
path = "Cargo.toml"
kind = "cargo-package"
primary = true
"#;
        let err = load_str(toml).unwrap_err();
        match err {
            ConfigError::ConflictingPrimaryManifest(source, primary) => {
                assert_eq!(source, "package.json");
                assert_eq!(primary, "Cargo.toml");
            }
            other => panic!("expected ConflictingPrimaryManifest, got {other:?}"),
        }
    }

    #[test]
    fn empty_manifests_returns_no_manifests_declared() {
        let toml = r#"
[git]
tag_prefix = "v"
"#;
        let err = load_str(toml).unwrap_err();
        assert!(matches!(err, ConfigError::NoManifestsDeclared));

        let toml_with_version = r#"
[version]
current_source = "Cargo.toml"
"#;
        let err = load_str(toml_with_version).unwrap_err();
        assert!(matches!(err, ConfigError::NoManifestsDeclared));
    }
}
