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
    #[error("preflight step '{0}' must be a string or inline table")]
    PreflightNotString(String),
    #[error("preflight step '{0}' is missing a string 'command' field")]
    PreflightMissingCommand(String),
    #[error("preflight timeout for '{0}' must be a positive integer")]
    PreflightInvalidTimeout(String),
    #[error("no cutver.toml or release.toml found in '{0}' or any parent directory")]
    NotFound(String),
    #[error("no manifests declared: at least one [[manifest]] entry is required")]
    NoManifestsDeclared,
    #[error("multiple manifests are marked with primary = true: only one manifest may be primary")]
    MultiplePrimaryManifests,
    #[error("conflicting primary manifest: [version] specifies '{0}' but '{1}' is marked as primary")]
    ConflictingPrimaryManifest(String, String),
}
#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(skip)]
    pub root_dir: PathBuf,
    #[serde(default)]
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
#[derive(Debug, Clone, Default, Deserialize)]
pub struct VersionSection {
    #[serde(alias = "source", default)]
    pub current_source: String,
    #[serde(default = "default_strategy")]
    pub strategy: String,
}

fn default_strategy() -> String {
    "manual".into()
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ManifestKind {
    Json {
        field: String,
    },
    #[serde(alias = "toml")]
    CargoPackage,
    Gradle {
        version_name_field: String,
        version_code_field: String,
    },
    Regex {
        pattern: String,
        replacement: String,
    },
}

#[derive(Debug, Clone, Deserialize)]
pub struct Manifest {
    pub path: String,
    #[serde(default)]
    pub primary: bool,
    #[serde(flatten)]
    pub kind: ManifestKind,
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
#[derive(Debug, Clone, Deserialize)]
pub struct Changelog {
    pub path: Option<String>,
    #[serde(default = "default_format")]
    pub format: String,
    #[serde(default = "default_entry_template")]
    pub entry_template: String,
    #[serde(default = "default_changelog_mode")]
    pub mode: String,
    #[serde(default = "default_include_scopes")]
    pub include_scopes: bool,
    #[serde(default = "default_fallback_entry")]
    pub fallback_entry: String,
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
            mode: default_changelog_mode(),
            include_scopes: default_include_scopes(),
            fallback_entry: default_fallback_entry(),
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
fn default_changelog_mode() -> String {
    "conventional".into()
}
fn default_include_scopes() -> bool {
    true
}
fn default_fallback_entry() -> String {
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
    let root = path.parent().unwrap_or_else(|| Path::new("."));
    config.root_dir = canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    deduce_current_source(&mut config)?;
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
        std::env::current_dir().map_err(ConfigError::Read)?.join(start)
    };
    let start = canonicalize(&start).unwrap_or(start);
    let mut dir = Some(start.as_path());
    while let Some(d) = dir {
        let cutver_candidate = d.join("cutver.toml");
        if cutver_candidate.is_file() {
            return load(cutver_candidate);
        }
        let release_candidate = d.join("release.toml");
        if release_candidate.is_file() {
            return load(release_candidate);
        }
        if d.join(".git").exists() {
            break;
        }
        dir = d.parent();
    }
    Err(ConfigError::NotFound(start.display().to_string()))
}
fn canonicalize(path: impl AsRef<Path>) -> io::Result<PathBuf> {
    let p = fs::canonicalize(path)?;
    Ok(strip_verbatim_prefix(p))
}

pub(crate) fn strip_verbatim_prefix(path: PathBuf) -> PathBuf {
    let s = match path.to_str() {
        Some(s) => s,
        None => return path,
    };
    if s.len() >= 8 && s[..8].eq_ignore_ascii_case(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{}", &s[8..]));
    }
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        let bytes = rest.as_bytes();
        if bytes.len() >= 2
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && (bytes.len() == 2 || bytes[2] == b'\\' || bytes[2] == b'/')
        {
            return PathBuf::from(rest);
        }
    }
    path
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

fn deduce_current_source(config: &mut Config) -> Result<(), ConfigError> {
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
    Ok(())
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
        canonicalize(&d).unwrap_or(d)
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

    #[test]
    fn discover_stops_at_git_boundary() {
        let parent = tmp("cutver-git-bound-parent");
        write(
            &parent,
            "release.toml",
            "[version]\ncurrent_source = \"a\"\n[[manifest]]\npath = \"a\"\nkind = \"cargo-package\"\n",
        );
        write(&parent, "a", "");

        let repo = parent.join("repo");
        fs::create_dir_all(repo.join(".git")).unwrap();
        let sub = repo.join("sub").join("nested");
        fs::create_dir_all(&sub).unwrap();

        // When starting from sub inside a git repo that lacks release.toml,
        // discovery must stop at repo/.git and NOT find parent/release.toml.
        let err = discover(&sub).unwrap_err();
        assert!(matches!(err, ConfigError::NotFound(_)));

        // When starting directly at the repo root without release.toml
        let err = discover(&repo).unwrap_err();
        assert!(matches!(err, ConfigError::NotFound(_)));
    }

    #[test]
    fn discover_stops_at_git_file_boundary() {
        let parent = tmp("cutver-git-file-bound-parent");
        write(
            &parent,
            "release.toml",
            "[version]\ncurrent_source = \"a\"\n[[manifest]]\npath = \"a\"\nkind = \"cargo-package\"\n",
        );
        write(&parent, "a", "");

        let repo = parent.join("worktree");
        fs::create_dir_all(&repo).unwrap();
        write(&repo, ".git", "gitdir: /path/to/gitdir");
        let sub = repo.join("sub");
        fs::create_dir_all(&sub).unwrap();

        let err = discover(&sub).unwrap_err();
        assert!(matches!(err, ConfigError::NotFound(_)));
    }

    #[test]
    fn discover_from_subdirectory_finds_release_toml() {
        let repo = tmp("cutver-discover-sub");
        fs::create_dir_all(repo.join(".git")).unwrap();
        write(
            &repo,
            "release.toml",
            "[version]\ncurrent_source = \"a\"\n[[manifest]]\npath = \"a\"\nkind = \"cargo-package\"\n",
        );
        write(&repo, "a", "");

        let deep = repo.join("src").join("nested").join("deep");
        fs::create_dir_all(&deep).unwrap();

        let c = discover(&deep).unwrap();
        assert_eq!(c.root_dir, repo);
        assert_eq!(c.version.current_source, repo.join("a").to_string_lossy().to_string());
    }

    #[test]
    fn strip_verbatim_windows_prefixes() {
        assert_eq!(
            strip_verbatim_prefix(PathBuf::from(r"\\?\C:\Users\foo\project")),
            PathBuf::from(r"C:\Users\foo\project")
        );
        assert_eq!(
            strip_verbatim_prefix(PathBuf::from(r"\\?\c:\Users\foo\project")),
            PathBuf::from(r"c:\Users\foo\project")
        );
        assert_eq!(strip_verbatim_prefix(PathBuf::from(r"\\?\C:")), PathBuf::from("C:"));
        assert_eq!(strip_verbatim_prefix(PathBuf::from(r"\\?\C:\")), PathBuf::from(r"C:\"));
        assert_eq!(
            strip_verbatim_prefix(PathBuf::from(r"\\?\UNC\server\share\path")),
            PathBuf::from(r"\\server\share\path")
        );
        assert_eq!(
            strip_verbatim_prefix(PathBuf::from(r"\\?\unc\server\share\path")),
            PathBuf::from(r"\\server\share\path")
        );
        assert_eq!(
            strip_verbatim_prefix(PathBuf::from("/unix/absolute/path")),
            PathBuf::from("/unix/absolute/path")
        );
        assert_eq!(
            strip_verbatim_prefix(PathBuf::from(r"\\?\Volume{b75e2c83-0000-0000-0000-602200000000}\foo")),
            PathBuf::from(r"\\?\Volume{b75e2c83-0000-0000-0000-602200000000}\foo")
        );
    }

    #[cfg(unix)]
    #[test]
    fn canonicalize_symlinked_root() {
        let repo = tmp("cutver-symlink-target");
        fs::create_dir_all(repo.join(".git")).unwrap();
        write(
            &repo,
            "release.toml",
            "[version]\ncurrent_source = \"a\"\n[[manifest]]\npath = \"a\"\nkind = \"cargo-package\"\n",
        );
        write(&repo, "a", "");

        let symlink_dir = std::env::temp_dir().join(tmp_id("cutver-symlink-src"));
        let _ = fs::remove_file(&symlink_dir);
        let _ = fs::remove_dir_all(&symlink_dir);
        std::os::unix::fs::symlink(&repo, &symlink_dir).unwrap();

        let c = load(symlink_dir.join("release.toml")).unwrap();
        assert_eq!(c.root_dir, repo);

        let c = discover(&symlink_dir).unwrap();
        assert_eq!(c.root_dir, repo);

        let sub = repo.join("sub");
        fs::create_dir_all(&sub).unwrap();
        let symlink_sub = std::env::temp_dir().join(tmp_id("cutver-symlink-sub"));
        let _ = fs::remove_file(&symlink_sub);
        let _ = fs::remove_dir_all(&symlink_sub);
        std::os::unix::fs::symlink(&sub, &symlink_sub).unwrap();

        let c = discover(&symlink_sub).unwrap();
        assert_eq!(c.root_dir, repo);

        let _ = fs::remove_file(&symlink_dir);
        let _ = fs::remove_file(&symlink_sub);
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

    #[test]
    fn discover_prioritizes_cutver_toml_over_release_toml() {
        let d = tmp("cutver-cfg-precedence");
        write(
            &d,
            "cutver.toml",
            "[[manifest]]\npath = \"cutver-manifest\"\nkind = \"cargo-package\"\n",
        );
        write(
            &d,
            "release.toml",
            "[[manifest]]\npath = \"release-manifest\"\nkind = \"cargo-package\"\n",
        );
        write(&d, "cutver-manifest", "");
        write(&d, "release-manifest", "");

        let c = discover(&d).unwrap();
        assert_eq!(
            c.version.current_source,
            d.join("cutver-manifest").to_string_lossy().to_string()
        );
    }

    #[test]
    fn discover_falls_back_to_release_toml_when_cutver_toml_absent() {
        let d = tmp("cutver-cfg-fallback");
        write(
            &d,
            "release.toml",
            "[[manifest]]\npath = \"release-manifest\"\nkind = \"cargo-package\"\n",
        );
        write(&d, "release-manifest", "");

        let c = discover(&d).unwrap();
        assert_eq!(
            c.version.current_source,
            d.join("release-manifest").to_string_lossy().to_string()
        );
    }

    #[test]
    fn changelog_defaults_and_custom_config() {
        let default_cl = Changelog::default();
        assert_eq!(default_cl.format, "keep-a-changelog");
        assert_eq!(default_cl.entry_template, "Maintenance and updates.");
        assert_eq!(default_cl.mode, "conventional");
        assert!(default_cl.include_scopes);
        assert_eq!(default_cl.fallback_entry, "Maintenance and updates.");

        let toml = r#"
[[manifest]]
path = "Cargo.toml"
kind = "cargo-package"

[changelog]
path = "CHANGELOG.md"
mode = "template"
include_scopes = false
fallback_entry = "Custom fallback notes."
"#;
        let c = load_str(toml).unwrap();
        assert_eq!(c.changelog.mode, "template");
        assert!(!c.changelog.include_scopes);
        assert_eq!(c.changelog.fallback_entry, "Custom fallback notes.");
    }
}
