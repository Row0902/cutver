use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use thiserror::Error;

use super::discovery::{DiscoveredKind, DiscoveredManifest, DiscoveryResult, discover_project};

#[derive(Debug, Error)]
pub enum InitError {
    #[error(
        "cutver.toml already exists at '{path}'.\n  To discover and add new manifests without overwriting existing settings:\n    cutver init --update\n  To overwrite the existing configuration:\n    cutver init --force"
    )]
    AlreadyExists { path: String },

    #[error("cannot update: '{path}' does not exist.\n  Run 'cutver init' to generate a new configuration.")]
    NotFoundForUpdate { path: String },

    #[error("failed to read existing '{path}': {source}")]
    ReadConfig { path: String, source: io::Error },

    #[error("failed to parse existing '{path}': {source}")]
    ParseConfig { path: String, source: toml_edit::TomlError },

    #[error("failed to discover manifests in '{path}': {source}")]
    Discovery { path: String, source: io::Error },

    #[error("failed to write '{path}': {source}")]
    WriteFile { path: String, source: crate::atomic::Error },

    #[error("io error: {0}")]
    Io(#[from] io::Error),
}

pub const STARTER_CHANGELOG: &str = "# Changelog

All notable changes to this project will be documented in this file.
Format based on [Keep a Changelog](https://keepachangelog.com).

## [Unreleased]
";

pub const DEFAULT_RELEASE_TEMPLATE: &str = r#"## [{{ tag }}] - {{ date }}

{%- if breaking %}
### ⚠️ Breaking Changes
{% for c in commits if c.is_breaking -%}
- {{ ('**' ~ c.scope ~ '**: ') if c.scope }}{{ c.clean_description or c.description }}{% if c.pr_number %} in [#{{ c.pr_number }}]({{ c.pr_url }}){% endif %}{% if c.short_hash and c.commit_url %} ([{{ c.short_hash }}]({{ c.commit_url }})){% elif c.short_hash %} ({{ c.short_hash }}){% endif %}{% if c.author %} by @{{ c.author }}{% endif %}
{% endfor %}
{%- endif %}

{%- if features %}
### 🚀 Features & Enhancements
{% for c in commits if c.commit_type == 'feat' and not c.is_breaking -%}
- {{ ('**' ~ c.scope ~ '**: ') if c.scope }}{{ c.clean_description or c.description }}{% if c.pr_number %} in [#{{ c.pr_number }}]({{ c.pr_url }}){% endif %}{% if c.short_hash and c.commit_url %} ([{{ c.short_hash }}]({{ c.commit_url }})){% elif c.short_hash %} ({{ c.short_hash }}){% endif %}{% if c.author %} by @{{ c.author }}{% endif %}
{% endfor %}
{%- endif %}

{%- if fixes %}
### 🐛 Bug Fixes
{% for c in commits if c.commit_type == 'fix' and not c.is_breaking -%}
- {{ ('**' ~ c.scope ~ '**: ') if c.scope }}{{ c.clean_description or c.description }}{% if c.pr_number %} in [#{{ c.pr_number }}]({{ c.pr_url }}){% endif %}{% if c.short_hash and c.commit_url %} ([{{ c.short_hash }}]({{ c.commit_url }})){% elif c.short_hash %} ({{ c.short_hash }}){% endif %}{% if c.author %} by @{{ c.author }}{% endif %}
{% endfor %}
{%- endif %}

{%- if perf %}
### ⚡ Performance Improvements
{% for c in commits if c.commit_type == 'perf' and not c.is_breaking -%}
- {{ ('**' ~ c.scope ~ '**: ') if c.scope }}{{ c.clean_description or c.description }}{% if c.pr_number %} in [#{{ c.pr_number }}]({{ c.pr_url }}){% endif %}{% if c.short_hash and c.commit_url %} ([{{ c.short_hash }}]({{ c.commit_url }})){% elif c.short_hash %} ({{ c.short_hash }}){% endif %}{% if c.author %} by @{{ c.author }}{% endif %}
{% endfor %}
{%- endif %}

{%- if refactor %}
### 🔄 Code Refactoring
{% for c in commits if c.commit_type == 'refactor' and not c.is_breaking -%}
- {{ ('**' ~ c.scope ~ '**: ') if c.scope }}{{ c.clean_description or c.description }}{% if c.pr_number %} in [#{{ c.pr_number }}]({{ c.pr_url }}){% endif %}{% if c.short_hash and c.commit_url %} ([{{ c.short_hash }}]({{ c.commit_url }})){% elif c.short_hash %} ({{ c.short_hash }}){% endif %}{% if c.author %} by @{{ c.author }}{% endif %}
{% endfor %}
{%- endif %}

{%- if docs %}
### 📚 Documentation
{% for c in commits if c.commit_type == 'docs' and not c.is_breaking -%}
- {{ ('**' ~ c.scope ~ '**: ') if c.scope }}{{ c.clean_description or c.description }}{% if c.pr_number %} in [#{{ c.pr_number }}]({{ c.pr_url }}){% endif %}{% if c.short_hash and c.commit_url %} ([{{ c.short_hash }}]({{ c.commit_url }})){% elif c.short_hash %} ({{ c.short_hash }}){% endif %}{% if c.author %} by @{{ c.author }}{% endif %}
{% endfor %}
{%- endif %}

{%- if maintenance %}
### 🛠️ Maintenance & Dependencies
{% for c in commits if c.commit_type in ['chore', 'build', 'ci', 'test'] and not c.is_breaking -%}
- {{ ('**' ~ c.scope ~ '**: ') if c.scope }}{{ c.clean_description or c.description }}{% if c.pr_number %} in [#{{ c.pr_number }}]({{ c.pr_url }}){% endif %}{% if c.short_hash and c.commit_url %} ([{{ c.short_hash }}]({{ c.commit_url }})){% elif c.short_hash %} ({{ c.short_hash }}){% endif %}{% if c.author %} by @{{ c.author }}{% endif %}
{% endfor %}
{%- endif %}

{%- if other %}
### 📦 Other Changes
{{ other }}
{%- endif %}

{%- if contributors %}
### 👥 Contributors
{% for author in contributors -%}
- @{{ author }}
{% endfor %}
{%- endif %}

{%- if compare_url %}
---
**Full Changelog**: {{ compare_url }}
{%- endif %}
"#;

pub const DEFAULT_TEMPLATE_PATH: &str = ".github/templates/cutver/RELEASE.md";

pub fn format_manifest_entry(m: &DiscoveredManifest) -> String {
    let mut s = String::new();
    s.push_str("[[manifest]]\n");
    s.push_str(&format!("path = \"{}\"\n", m.path));
    match &m.kind {
        DiscoveredKind::CargoPackage => {
            s.push_str("kind = \"cargo-package\"\n");
        }
        DiscoveredKind::Json { field } => {
            s.push_str("kind = \"json\"\n");
            s.push_str(&format!("field = \"{field}\"\n"));
        }
        DiscoveredKind::Gradle {
            version_name_field,
            version_code_field,
        } => {
            s.push_str("kind = \"gradle\"\n");
            s.push_str(&format!("version_name_field = \"{version_name_field}\"\n"));
            s.push_str(&format!("version_code_field = \"{version_code_field}\"\n"));
        }
        DiscoveredKind::Pyproject => {
            s.push_str("kind = \"pyproject\"\n");
        }
    }
    s
}

pub fn generate_fresh_config(discovery: &DiscoveryResult, no_template: bool) -> String {
    let mut out = String::new();
    out.push_str("# cutver.toml - release orchestration configuration\n");
    out.push_str("# For full documentation, see https://github.com/cutver/cutver\n");

    if discovery.manifests.is_empty() {
        out.push_str("\n# No manifests were automatically detected during init.\n");
        out.push_str("# Declare your project manifests below (the first is the source of truth):\n");
        out.push_str("# [[manifest]]\n");
        out.push_str("# path = \"Cargo.toml\"\n");
        out.push_str("# kind = \"cargo-package\"\n");
    } else {
        out.push_str("\n# The first declared manifest serves as the primary source of truth for versions\n");
        for m in &discovery.manifests {
            out.push('\n');
            out.push_str(&format_manifest_entry(m));
        }
    }

    let has_preflight = discovery.hints.has_rust || discovery.hints.has_node;
    if has_preflight {
        out.push_str("\n[preflight]\n");
        if discovery.hints.has_rust {
            out.push_str("check = \"cargo check --workspace\"\n");
        }
        if discovery.hints.has_node {
            out.push_str("test = \"npm test\"\n");
        }
        out.push_str("default_timeout = 300\n");
    }

    out.push_str("\n[changelog]\n");
    out.push_str("path = \"CHANGELOG.md\"\n");
    out.push_str("format = \"keep-a-changelog\"\n");
    out.push_str("mode = \"conventional\"\n");
    if !no_template {
        out.push_str("template_file = \".github/templates/cutver/RELEASE.md\"\n");
    }

    out.push_str("\n[git]\n");
    out.push_str("tag_prefix = \"v\"\n");
    out.push_str("require_clean_tree = true\n");
    out.push_str("commit_message = \"chore(release): v{version} [skip ci]\"\n");
    out.push_str("require_branch = \"main\"\n");

    let has_hooks = discovery.hints.has_rust || discovery.hints.has_uv_lock;
    if has_hooks {
        out.push_str("\n[hooks]\n");
        if discovery.hints.has_rust && discovery.hints.has_uv_lock {
            out.push_str("post_bump = \"cargo check --workspace && uv lock\"\n");
        } else if discovery.hints.has_rust {
            out.push_str("post_bump = \"cargo check --workspace\"\n");
        } else if discovery.hints.has_uv_lock {
            out.push_str("post_bump = \"uv lock\"\n");
        }
    }

    out.push_str("\n[publish]\n");
    out.push_str("push = true\n");

    out
}

pub fn update_existing_config(
    config_path: &Path,
    discovered: &[DiscoveredManifest],
) -> Result<Vec<DiscoveredManifest>, InitError> {
    let text = fs::read_to_string(config_path).map_err(|e| InitError::ReadConfig {
        path: config_path.display().to_string(),
        source: e,
    })?;

    let doc: toml_edit::DocumentMut = text.parse().map_err(|e| InitError::ParseConfig {
        path: config_path.display().to_string(),
        source: e,
    })?;

    let mut existing_paths = std::collections::HashSet::new();
    if let Some(manifests) = doc.get("manifest").and_then(|m| m.as_array_of_tables()) {
        for m in manifests.iter() {
            if let Some(p) = m.get("path").and_then(|p| p.as_str()) {
                existing_paths.insert(p.to_string());
            }
        }
    }
    if let Some(arr) = doc.get("manifest").and_then(|m| m.as_array()) {
        for v in arr.iter() {
            if let Some(tbl) = v.as_inline_table()
                && let Some(p) = tbl.get("path").and_then(|p| p.as_str())
            {
                existing_paths.insert(p.to_string());
            }
        }
    }

    let new_manifests: Vec<DiscoveredManifest> = discovered
        .iter()
        .filter(|m| !existing_paths.contains(&m.path))
        .cloned()
        .collect();

    if new_manifests.is_empty() {
        return Ok(Vec::new());
    }

    let mut updated_text = text;
    if !updated_text.ends_with('\n') {
        updated_text.push('\n');
    }
    for m in &new_manifests {
        updated_text.push('\n');
        updated_text.push_str(&format_manifest_entry(m));
    }

    let _ = updated_text
        .parse::<toml_edit::DocumentMut>()
        .map_err(|e| InitError::ParseConfig {
            path: config_path.display().to_string(),
            source: e,
        })?;

    crate::atomic::write_atomic(config_path, updated_text.as_bytes()).map_err(|e| InitError::WriteFile {
        path: config_path.display().to_string(),
        source: e,
    })?;

    Ok(new_manifests)
}

fn describe_kind(kind: &DiscoveredKind) -> &'static str {
    match kind {
        DiscoveredKind::CargoPackage => "cargo-package",
        DiscoveredKind::Json { .. } => "json: version",
        DiscoveredKind::Gradle { .. } => "gradle",
        DiscoveredKind::Pyproject => "pyproject",
    }
}

fn print_fresh_summary(target_dir: &Path, discovery: &DiscoveryResult, created_files: &[&str]) {
    if discovery.manifests.is_empty() {
        println!("⚠ Initialized cutver in {}\n", target_dir.display());
        println!("No package manifests were automatically discovered.");
        println!("A starter configuration has been created. Declare your manifests under [[manifest]].\n");
    } else {
        println!("✔ Initialized cutver configuration in {}\n", target_dir.display());
        println!("Discovered manifests:");
        for (i, m) in discovery.manifests.iter().enumerate() {
            let primary_badge = if i == 0 { " [primary source of truth]" } else { "" };
            println!("  ✔ {} ({}){}", m.path, describe_kind(&m.kind), primary_badge);
        }
        println!();
    }

    println!("Created files:");
    for f in created_files {
        println!("  ✔ {f}");
    }
    println!();

    print_next_steps();
}

fn print_next_steps() {
    println!("Next steps:");
    println!("  1. Validate your configuration:");
    println!("     cutver doctor");
    println!("  2. Simulate your first release:");
    println!("     cutver bump auto --dry-run");
}

pub fn run_init(path: Option<PathBuf>, update: bool, force: bool, no_template: bool) -> Result<(), InitError> {
    let target_dir = match path {
        Some(p) => {
            if p.is_relative() {
                std::env::current_dir()?.join(p)
            } else {
                p
            }
        }
        None => std::env::current_dir()?,
    };

    let config_path = target_dir.join("cutver.toml");
    let changelog_path = target_dir.join("CHANGELOG.md");
    let template_path = target_dir.join(DEFAULT_TEMPLATE_PATH);

    let discovery = discover_project(&target_dir).map_err(|e| InitError::Discovery {
        path: target_dir.display().to_string(),
        source: e,
    })?;

    if update {
        if !config_path.is_file() {
            return Err(InitError::NotFoundForUpdate {
                path: config_path.display().to_string(),
            });
        }
        let added = update_existing_config(&config_path, &discovery.manifests)?;
        if added.is_empty() {
            println!("ℹ cutver.toml is already up to date (no new manifests discovered).");
        } else {
            println!("✔ Updated cutver.toml with {} new manifest(s):\n", added.len());
            for m in &added {
                println!("  ✔ {} ({})", m.path, describe_kind(&m.kind));
            }
            println!();
            print_next_steps();
        }
        return Ok(());
    }

    if config_path.exists() && !force {
        return Err(InitError::AlreadyExists {
            path: config_path.display().to_string(),
        });
    }

    let config_content = generate_fresh_config(&discovery, no_template);
    crate::atomic::write_atomic(&config_path, config_content.as_bytes()).map_err(|e| InitError::WriteFile {
        path: config_path.display().to_string(),
        source: e,
    })?;

    let mut created_files = vec!["cutver.toml"];
    if !changelog_path.exists() {
        crate::atomic::write_atomic(&changelog_path, STARTER_CHANGELOG.as_bytes()).map_err(|e| {
            InitError::WriteFile {
                path: changelog_path.display().to_string(),
                source: e,
            }
        })?;
        created_files.push("CHANGELOG.md");
    }

    if !no_template && (!template_path.exists() || force) {
        if let Some(parent) = template_path.parent() {
            fs::create_dir_all(parent)?;
        }
        crate::atomic::write_atomic(&template_path, DEFAULT_RELEASE_TEMPLATE.as_bytes()).map_err(|e| {
            InitError::WriteFile {
                path: template_path.display().to_string(),
                source: e,
            }
        })?;
        created_files.push(DEFAULT_TEMPLATE_PATH);
    }

    print_fresh_summary(&target_dir, &discovery, &created_files);
    Ok(())
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

        fn read(&self, rel: &str) -> String {
            fs::read_to_string(self.path.join(rel)).unwrap()
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn fresh_init_creates_files_and_config() {
        let td = TestDir::new("scaffold-fresh");
        td.write("Cargo.toml", "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n");

        run_init(Some(td.path.clone()), false, false, false).unwrap();

        assert!(td.path.join("cutver.toml").is_file());
        assert!(td.path.join("CHANGELOG.md").is_file());
        assert!(td.path.join(".github/templates/cutver/RELEASE.md").is_file());

        let cfg_text = td.read("cutver.toml");
        assert!(cfg_text.contains("commit_message = \"chore(release): v{version} [skip ci]\""));
        assert!(cfg_text.contains("require_branch = \"main\""));
        assert!(cfg_text.contains("push = true"));
        assert!(cfg_text.contains("kind = \"cargo-package\""));
        assert!(cfg_text.contains("check = \"cargo check --workspace\""));
        assert!(cfg_text.contains("post_bump = \"cargo check --workspace\""));
        assert!(cfg_text.contains("template_file = \".github/templates/cutver/RELEASE.md\""));

        let changelog = td.read("CHANGELOG.md");
        assert!(changelog.contains("## [Unreleased]"));

        let template = td.read(".github/templates/cutver/RELEASE.md");
        assert!(template.contains("## [{{ tag }}] - {{ date }}"));
    }

    #[test]
    fn fresh_init_no_template_flag() {
        let td = TestDir::new("scaffold-no-template");
        td.write("Cargo.toml", "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n");

        run_init(Some(td.path.clone()), false, false, true).unwrap();

        assert!(td.path.join("cutver.toml").is_file());
        assert!(td.path.join("CHANGELOG.md").is_file());
        assert!(!td.path.join(".github/templates/cutver/RELEASE.md").exists());

        let cfg_text = td.read("cutver.toml");
        assert!(!cfg_text.contains("template_file"));
    }

    #[test]
    fn fresh_init_fails_if_cutver_toml_exists() {
        let td = TestDir::new("scaffold-exists");
        td.write("cutver.toml", "# existing\n");

        let err = run_init(Some(td.path.clone()), false, false, false).unwrap_err();
        assert!(matches!(err, InitError::AlreadyExists { .. }));
    }

    #[test]
    fn fresh_init_overwrites_with_force() {
        let td = TestDir::new("scaffold-force");
        td.write("cutver.toml", "# old\n");
        td.write("package.json", r#"{"name": "test", "version": "1.0.0"}"#);

        run_init(Some(td.path.clone()), false, true, false).unwrap();

        let cfg_text = td.read("cutver.toml");
        assert!(cfg_text.contains("kind = \"json\""));
        assert!(cfg_text.contains("field = \"version\""));
        assert!(!cfg_text.contains("# old"));
    }

    #[test]
    fn update_appends_new_manifest_and_preserves_customizations() {
        let td = TestDir::new("scaffold-update");
        let initial_cfg = r#"# Custom user header
[version]
strategy = "conventional"

[[manifest]]
path = "Cargo.toml"
kind = "cargo-package"

[preflight]
custom_lint = "cargo clippy"

[git]
tag_prefix = "rel-"
"#;
        td.write("cutver.toml", initial_cfg);
        td.write("Cargo.toml", "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n");
        td.write(
            "packages/client/package.json",
            r#"{"name": "client", "version": "0.1.0"}"#,
        );

        run_init(Some(td.path.clone()), true, false, false).unwrap();

        let updated_cfg = td.read("cutver.toml");
        assert!(updated_cfg.contains("# Custom user header"));
        assert!(updated_cfg.contains("custom_lint = \"cargo clippy\""));
        assert!(updated_cfg.contains("tag_prefix = \"rel-\""));
        assert!(updated_cfg.contains("path = \"packages/client/package.json\""));
    }

    #[test]
    fn update_noop_when_all_manifests_already_present() {
        let td = TestDir::new("scaffold-noop");
        let initial_cfg = r#"[version]
strategy = "conventional"

[[manifest]]
path = "Cargo.toml"
kind = "cargo-package"
"#;
        td.write("cutver.toml", initial_cfg);
        td.write("Cargo.toml", "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n");

        run_init(Some(td.path.clone()), true, false, false).unwrap();
        assert_eq!(td.read("cutver.toml"), initial_cfg);
    }
}
