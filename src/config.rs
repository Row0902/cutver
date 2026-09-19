use serde::Deserialize;
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
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
    #[error("preflight step '{0}' must be a string or inline table")]
    PreflightNotString(String),
    #[error("preflight step '{0}' is missing a string 'command' field")]
    PreflightMissingCommand(String),
    #[error("preflight timeout for '{0}' must be a positive integer")]
    PreflightInvalidTimeout(String),
    #[error("no release.toml found in '{0}' or any parent directory")]
    NotFound(String),
}
#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(skip)]
    pub root_dir: PathBuf,
    pub version: VersionSection,
    #[serde(default)]
    pub manifest: Vec<Manifest>,
    #[serde(skip)]
    pub preflight: PreflightSteps,
    #[serde(skip)]
    pub preflight_default_timeout: Option<u64>,
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
#[derive(Debug, Clone)]
pub struct PreflightCommand {
    pub command: String,
    pub timeout: Option<u64>,
}

/// Ordered `[preflight]` steps as declared in `release.toml`.
pub type PreflightSteps = Vec<(String, PreflightCommand)>;

/// Global `[preflight] default_timeout`.
pub type PreflightDefault = Option<u64>;
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
    fn default() -> Self {
        Self {
            path: None,
            format: default_format(),
            entry_template: default_entry_template(),
        }
    }
}
impl Default for Git {
    fn default() -> Self {
        Self {
            tag_prefix: default_tag_prefix(),
            commit_message: default_commit_message(),
            require_clean_tree: default_require_clean_tree(),
            require_branch: None,
        }
    }
}
fn default_format() -> String {
    "keep-a-changelog".into()
}
fn default_entry_template() -> String {
    "Maintenance and updates.".into()
}
fn default_tag_prefix() -> String {
    "v".into()
}
fn default_commit_message() -> String {
    "chore(release): v{version}".into()
}
fn default_require_clean_tree() -> bool {
    true
}

pub fn load(path: impl AsRef<Path>) -> Result<Config, ConfigError> {
    let raw = path.as_ref();
    let path = if raw.is_absolute() {
        raw.to_path_buf()
    } else {
        std::env::current_dir().map_err(ConfigError::Read)?.join(raw)
    };
    let text = fs::read_to_string(&path)?;
    let mut config: Config = toml::from_str(&text)?;
    let (preflight, default_timeout) = parse_preflight(&text)?;
    config.preflight = preflight;
    config.preflight_default_timeout = default_timeout;
    config.root_dir = path.parent().unwrap_or_else(|| Path::new(".")).to_path_buf();
    config.version.current_source = resolve(&config.root_dir, &config.version.current_source);
    for m in &mut config.manifest {
        m.path = resolve(&config.root_dir, &m.path);
    }
    if let Some(ref mut p) = config.changelog.path {
        *p = resolve(&config.root_dir, p);
    }
    validate(&config)?;
    Ok(config)
}
pub fn discover(start_dir: impl AsRef<Path>) -> Result<Config, ConfigError> {
    let start = start_dir.as_ref();
    let start = if start.is_absolute() {
        start.to_path_buf()
    } else {
        std::env::current_dir()?.join(start)
    };
    let mut dir = Some(start.as_path());
    while let Some(d) = dir {
        let candidate = d.join("release.toml");
        if candidate.is_file() {
            return load(candidate);
        }
        dir = d.parent();
    }
    Err(ConfigError::NotFound(start.display().to_string()))
}
fn resolve(root: &Path, path: &str) -> String {
    let p = Path::new(path);
    if p.is_absolute() {
        path.into()
    } else {
        root.join(p).display().to_string()
    }
}

fn parse_preflight(text: &str) -> Result<(PreflightSteps, PreflightDefault), ConfigError> {
    let doc = text.parse::<toml_edit::DocumentMut>()?;
    let mut cmds = Vec::new();
    let mut default_timeout = None;
    if let Some(table) = doc.get("preflight").and_then(|v| v.as_table()) {
        for (k, v) in table.iter() {
            if k == "default_timeout" {
                default_timeout = Some(parse_timeout(
                    k,
                    v.as_value()
                        .ok_or_else(|| ConfigError::PreflightInvalidTimeout(k.into()))?,
                )?);
                continue;
            }
            cmds.push((k.into(), parse_step(k, v)?));
        }
    }
    if let Some(t) = default_timeout {
        for (_, c) in &mut cmds {
            if c.timeout.is_none() {
                c.timeout = Some(t);
            }
        }
    }
    Ok((cmds, default_timeout))
}
fn parse_step(name: &str, v: &toml_edit::Item) -> Result<PreflightCommand, ConfigError> {
    if let Some(s) = v.as_str() {
        return Ok(PreflightCommand {
            command: s.into(),
            timeout: None,
        });
    }
    let tbl = v
        .as_inline_table()
        .ok_or_else(|| ConfigError::PreflightNotString(name.into()))?;
    let cmd = tbl
        .get("command")
        .and_then(|c| c.as_str())
        .ok_or_else(|| ConfigError::PreflightMissingCommand(name.into()))?;
    let timeout = tbl.get("timeout").map(|t| parse_timeout(name, t)).transpose()?;
    Ok(PreflightCommand {
        command: cmd.into(),
        timeout,
    })
}
fn parse_timeout(name: &str, v: &toml_edit::Value) -> Result<u64, ConfigError> {
    let n = v
        .as_integer()
        .ok_or_else(|| ConfigError::PreflightInvalidTimeout(name.into()))?;
    if n <= 0 {
        return Err(ConfigError::PreflightInvalidTimeout(name.into()));
    }
    n.try_into()
        .map_err(|_| ConfigError::PreflightInvalidTimeout(name.into()))
}

fn validate(config: &Config) -> Result<(), ConfigError> {
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
    if opt.is_none() {
        Err(ConfigError::MissingField(m.path.clone(), m.kind.clone(), field.into()))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    static TMP_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    fn tmp_id(prefix: &str) -> String {
        let n = TMP_SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        format!("{}-{}-{}", prefix, std::process::id(), n)
    }

    use super::*;
    fn load_str(s: &str) -> Result<Config, ConfigError> {
        let mut c: Config = toml::from_str(s).map_err(ConfigError::Parse)?;
        let (p, d) = parse_preflight(s)?;
        c.preflight = p;
        c.preflight_default_timeout = d;
        c.root_dir = std::env::current_dir().unwrap();
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
        assert!(matches!(
            load_str(&manifest("json", "")),
            Err(ConfigError::MissingField(_, _, _))
        ));
        assert!(matches!(
            load_str(&manifest("gradle", "")),
            Err(ConfigError::MissingField(_, _, _))
        ));
        assert!(matches!(
            load_str(&manifest("regex", "")),
            Err(ConfigError::MissingField(_, _, _))
        ));
        assert!(matches!(
            load_str(&manifest("unknown", "")),
            Err(ConfigError::UnknownKind(_, _))
        ));
        assert!(matches!(
            load_str(&(manifest("cargo-package", "") + "\n[preflight]\ntests = 123")),
            Err(ConfigError::PreflightNotString(_))
        ));
    }
    #[test]
    fn preflight_timeouts() {
        let cfg = load_str(&manifest(
            "cargo-package",
            r#"[preflight]
default_timeout = 5
a = "echo a"
b = { command = "echo b", timeout = 10 }
d = { command = "echo d" }
"#,
        ))
        .unwrap();
        let m: std::collections::HashMap<_, _> = cfg.preflight.into_iter().collect();
        assert_eq!(m["a"].timeout, Some(5));
        assert_eq!(m["b"].timeout, Some(10));
        assert_eq!(m["d"].timeout, Some(5));
        assert!(
            load_str(&manifest(
                "cargo-package",
                "[preflight]\nt = { command = \"x\", timeout = -1 }\n"
            ))
            .is_err()
        );
        assert!(
            load_str(&manifest(
                "cargo-package",
                "[preflight]\nt = { command = \"x\", timeout = \"nope\" }\n"
            ))
            .is_err()
        );
        assert!(load_str(&manifest("cargo-package", "[preflight]\nt = { timeout = 1 }\n")).is_err());
        assert!(load_str(&manifest("cargo-package", "[preflight]\ndefault_timeout = -1\n")).is_err());
    }
    fn tmp(p: &str) -> PathBuf {
        let d = std::env::temp_dir().join(tmp_id(p));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }
    fn write(d: &Path, n: &str, t: &str) {
        fs::write(d.join(n), t).unwrap();
    }
    #[test]
    fn path_resolution_and_discovery() {
        let d = tmp("cutver-cfg-rel");
        write(
            &d,
            "release.toml",
            "[version]\ncurrent_source = \"a\"\n[[manifest]]\npath = \"a\"\nkind = \"cargo-package\"\n[changelog]\npath = \"CHANGELOG.md\"\n",
        );
        write(&d, "a", "");
        let c = load(d.join("release.toml")).unwrap();
        let a = d.join("a").to_string_lossy().to_string();
        assert_eq!((c.version.current_source, c.manifest[0].path.clone()), (a.clone(), a));
        assert_eq!(
            c.changelog.path,
            Some(d.join("CHANGELOG.md").to_string_lossy().to_string())
        );

        let d = tmp("cutver-cfg-abs");
        let a = d.join("a").to_string_lossy().replace('\\', "/");
        write(
            &d,
            "release.toml",
            &format!("[version]\ncurrent_source = '{a}'\n[[manifest]]\npath = '{a}'\nkind = \"cargo-package\"\n"),
        );
        write(&d, "a", "");
        let c = load(d.join("release.toml")).unwrap();
        assert_eq!((c.version.current_source, c.manifest[0].path.clone()), (a.clone(), a));

        let d = tmp("cutver-cfg-discover");
        let s = d.join("sub");
        fs::create_dir_all(&s).unwrap();
        write(
            &d,
            "release.toml",
            "[version]\ncurrent_source = \"a\"\n[[manifest]]\npath = \"a\"\nkind = \"cargo-package\"\n",
        );
        write(&d, "a", "");
        let c = discover(&s).unwrap();
        assert_eq!(c.root_dir, d);
        assert_eq!(c.version.current_source, d.join("a").to_string_lossy().to_string());
    }
}
