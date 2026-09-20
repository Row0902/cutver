use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::preflight::parse_preflight;
use super::types::{Config, ConfigError};
use super::validation::{deduce_current_source, validate};

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

#[cfg(test)]
mod tests {
    use super::*;

    static TMP_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    fn tmp_id(prefix: &str) -> String {
        let n = TMP_SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        format!("{}-{}-{}", prefix, std::process::id(), n)
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
}
