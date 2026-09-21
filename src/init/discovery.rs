use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const MAX_DEPTH: usize = 5;

const IGNORED_DIR_NAMES: &[&str] = &[
    ".git",
    "target",
    "node_modules",
    "venv",
    ".venv",
    "dist",
    "build",
    ".tox",
    "__pycache__",
    ".cargo",
    "vendor",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoveredKind {
    CargoPackage,
    Json {
        field: String,
    },
    Gradle {
        version_name_field: String,
        version_code_field: String,
    },
    Pyproject,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredManifest {
    pub path: String,
    pub is_root: bool,
    pub kind: DiscoveredKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EcosystemHints {
    pub has_rust: bool,
    pub has_node: bool,
    pub has_python: bool,
    pub has_uv_lock: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryResult {
    pub manifests: Vec<DiscoveredManifest>,
    pub hints: EcosystemHints,
}

fn should_ignore_dir(name: &str) -> bool {
    name.starts_with('.') || IGNORED_DIR_NAMES.contains(&name)
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn scan_dir(
    root: &Path,
    current: &Path,
    depth: usize,
    manifests: &mut Vec<DiscoveredManifest>,
    hints: &mut EcosystemHints,
) -> io::Result<()> {
    let read_dir = match fs::read_dir(current) {
        Ok(rd) => rd,
        Err(e) if e.kind() == io::ErrorKind::PermissionDenied => return Ok(()),
        Err(e) => return Err(e),
    };

    let mut entries: Vec<fs::DirEntry> = Vec::new();
    for entry in read_dir {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        entries.push(entry);
    }
    entries.sort_by_key(|e| e.file_name());

    let mut subdirs: Vec<PathBuf> = Vec::new();
    let mut has_gradle_kts_in_dir = false;

    for entry in &entries {
        let name_os = entry.file_name();
        let name = name_os.to_string_lossy();
        let file_type = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };

        if file_type.is_dir() {
            if !should_ignore_dir(&name) && depth < MAX_DEPTH && !file_type.is_symlink() {
                subdirs.push(entry.path());
            }
            continue;
        }

        if !file_type.is_file() {
            continue;
        }

        match name.as_ref() {
            "Cargo.lock" => hints.has_rust = true,
            "package-lock.json" | "yarn.lock" | "pnpm-lock.yaml" | "bun.lockb" => hints.has_node = true,
            "uv.lock" => {
                hints.has_python = true;
                hints.has_uv_lock = true;
            }
            "build.gradle.kts" => has_gradle_kts_in_dir = true,
            _ => {}
        }

        let rel_path = match entry.path().strip_prefix(root) {
            Ok(p) => p.to_path_buf(),
            Err(_) => entry.path(),
        };
        let rel_str = normalize_path(&rel_path);
        let is_root = depth == 0;

        match name.as_ref() {
            "Cargo.toml" => {
                hints.has_rust = true;
                let is_virtual_workspace = match fs::read_to_string(entry.path()) {
                    Ok(content) => content.contains("[workspace]") && !content.contains("[package]"),
                    Err(_) => false,
                };
                if !is_virtual_workspace {
                    manifests.push(DiscoveredManifest {
                        path: rel_str,
                        is_root,
                        kind: DiscoveredKind::CargoPackage,
                    });
                }
            }
            "package.json" => {
                hints.has_node = true;
                manifests.push(DiscoveredManifest {
                    path: rel_str,
                    is_root,
                    kind: DiscoveredKind::Json {
                        field: "version".to_string(),
                    },
                });
            }
            "pyproject.toml" => {
                hints.has_python = true;
                manifests.push(DiscoveredManifest {
                    path: rel_str,
                    is_root,
                    kind: DiscoveredKind::Pyproject,
                });
            }
            "build.gradle.kts" => {
                manifests.push(DiscoveredManifest {
                    path: rel_str,
                    is_root,
                    kind: DiscoveredKind::Gradle {
                        version_name_field: "versionName".to_string(),
                        version_code_field: "versionCode".to_string(),
                    },
                });
            }
            "build.gradle" => {
                if !has_gradle_kts_in_dir {
                    manifests.push(DiscoveredManifest {
                        path: rel_str,
                        is_root,
                        kind: DiscoveredKind::Gradle {
                            version_name_field: "versionName".to_string(),
                            version_code_field: "versionCode".to_string(),
                        },
                    });
                }
            }
            "tauri.conf.json" => {
                manifests.push(DiscoveredManifest {
                    path: rel_str,
                    is_root: false,
                    kind: DiscoveredKind::Json {
                        field: "version".to_string(),
                    },
                });
            }
            _ => {}
        }
    }

    for subdir in subdirs {
        scan_dir(root, &subdir, depth + 1, manifests, hints)?;
    }

    Ok(())
}

fn manifest_rank(m: &DiscoveredManifest) -> (u8, usize, u8, &str) {
    let root_rank = if m.is_root { 0 } else { 1 };
    let depth = m.path.matches('/').count();
    let type_rank = match (&m.kind, m.path.as_str()) {
        (DiscoveredKind::Json { .. }, "package.json") => 0,
        (DiscoveredKind::CargoPackage, "Cargo.toml") => 1,
        (DiscoveredKind::Pyproject, _) => 2,
        (DiscoveredKind::Gradle { .. }, _) => 3,
        (DiscoveredKind::CargoPackage, _) => 4,
        (DiscoveredKind::Json { .. }, p) if !p.ends_with("tauri.conf.json") => 5,
        (DiscoveredKind::Json { .. }, _) => 6,
    };
    (root_rank, depth, type_rank, &m.path)
}

pub fn sort_manifests(manifests: &mut [DiscoveredManifest]) {
    manifests.sort_by(|a, b| manifest_rank(a).cmp(&manifest_rank(b)));
}

pub fn discover_project(root: &Path) -> io::Result<DiscoveryResult> {
    let mut manifests = Vec::new();
    let mut hints = EcosystemHints::default();

    scan_dir(root, root, 0, &mut manifests, &mut hints)?;
    sort_manifests(&mut manifests);

    Ok(DiscoveryResult { manifests, hints })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    static TMP_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    fn tmp_id(prefix: &str) -> String {
        let n = TMP_SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        format!("{}-{}-{}", prefix, std::process::id(), n)
    }

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(tmp_id(name));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Self { path }
        }

        fn write(&self, rel: &str, content: &str) {
            let p = self.path.join(rel);
            if let Some(parent) = p.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(p, content).unwrap();
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn discovers_cargo_root_and_subcrates() {
        let td = TestDir::new("disco-cargo");
        td.write("Cargo.toml", "[package]\nname = \"root\"\nversion = \"0.1.0\"\n");
        td.write(
            "crates/sub/Cargo.toml",
            "[package]\nname = \"sub\"\nversion = \"0.1.0\"\n",
        );
        td.write("target/debug/build/Cargo.toml", "[package]\nname = \"ignore\"\n");

        let res = discover_project(&td.path).unwrap();
        assert_eq!(res.manifests.len(), 2);
        assert_eq!(res.manifests[0].path, "Cargo.toml");
        assert!(res.manifests[0].is_root);
        assert_eq!(res.manifests[0].kind, DiscoveredKind::CargoPackage);

        assert_eq!(res.manifests[1].path, "crates/sub/Cargo.toml");
        assert!(!res.manifests[1].is_root);

        assert!(res.hints.has_rust);
        assert!(!res.hints.has_node);
    }

    #[test]
    fn discovers_polyglot_and_orders_root_first() {
        let td = TestDir::new("disco-poly");
        td.write("package.json", r#"{"name": "app", "version": "1.0.0"}"#);
        td.write("Cargo.toml", "[package]\nname = \"core\"\nversion = \"1.0.0\"\n");
        td.write("src-tauri/tauri.conf.json", r#"{"version": "1.0.0"}"#);
        td.write("android/build.gradle.kts", "versionName \"1.0.0\"\nversionCode 1\n");

        let res = discover_project(&td.path).unwrap();
        assert_eq!(res.manifests.len(), 4);
        assert_eq!(res.manifests[0].path, "package.json");
        assert_eq!(res.manifests[1].path, "Cargo.toml");
        assert_eq!(res.manifests[2].path, "android/build.gradle.kts");
        assert_eq!(res.manifests[3].path, "src-tauri/tauri.conf.json");

        assert!(res.hints.has_rust);
        assert!(res.hints.has_node);
    }

    #[test]
    fn skips_virtual_workspace_cargo_toml() {
        let td = TestDir::new("disco-vws");
        td.write("Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n");
        td.write(
            "crates/child/Cargo.toml",
            "[package]\nname = \"child\"\nversion = \"0.1.0\"\n",
        );

        let res = discover_project(&td.path).unwrap();
        assert_eq!(res.manifests.len(), 1);
        assert_eq!(res.manifests[0].path, "crates/child/Cargo.toml");
        assert!(res.hints.has_rust);
    }

    #[test]
    fn detects_python_and_uv_lock() {
        let td = TestDir::new("disco-py");
        td.write("pyproject.toml", "[project]\nversion = \"0.1.0\"\n");
        td.write("uv.lock", "version = 1\n");

        let res = discover_project(&td.path).unwrap();
        assert_eq!(res.manifests.len(), 1);
        assert_eq!(res.manifests[0].path, "pyproject.toml");
        assert_eq!(res.manifests[0].kind, DiscoveredKind::Pyproject);
        assert!(res.hints.has_python);
        assert!(res.hints.has_uv_lock);
    }

    #[test]
    fn ignores_specified_directories() {
        let td = TestDir::new("disco-ignore");
        td.write("node_modules/pkg/package.json", "{}");
        td.write(".git/hooks/package.json", "{}");
        td.write("venv/lib/pyproject.toml", "");
        td.write(".venv/lib/pyproject.toml", "");
        td.write("build/package.json", "{}");
        td.write("dist/package.json", "{}");
        td.write("vendor/bundle/Cargo.toml", "");
        td.write(".cargo/config.toml", "");

        let res = discover_project(&td.path).unwrap();
        assert!(res.manifests.is_empty());
    }
}
