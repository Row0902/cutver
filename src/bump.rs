use crate::changelog;
use crate::config::Config;
use crate::git;
use crate::manifest;
use crate::preflight;
use crate::semver_bump::{self, Bump};
use semver::Version;
use std::fs;
use std::io;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("git guard failed: {0}")]
    Git(#[from] git::Error),
    #[error("failed to read '{path}': {source}")]
    Read { path: String, source: io::Error },
    #[error("failed to write '{path}': {source}")]
    Write { path: String, source: io::Error },
    #[error("current source '{path}': {source}")]
    CurrentSource { path: String, source: manifest::Error },
    #[error("manifest '{path}': {source}")]
    Manifest { path: String, source: manifest::Error },
    #[error("preflight failed: {0}")]
    Preflight(#[from] preflight::Error),
    #[error("changelog failed: {0}")]
    Changelog(#[from] changelog::Error),
}

#[derive(Debug)]
pub struct Touched { pub path: String, pub old: String, pub new: String }

#[derive(Debug)]
pub struct Summary {
    pub source: String,
    pub current: Version,
    pub next: Version,
    pub dry_run: bool,
    pub preflight: Vec<preflight::Step>,
    pub touched: Vec<Touched>,
    pub changelog: Option<String>,
    pub commit_message: String,
    pub tag: String,
}

#[derive(Debug)]
pub struct Drift { pub path: String, pub expected: String, pub actual: String }

pub fn run(config: &Config, bump_kind: Bump, dry_run: bool, skip_preflight: &[String]) -> Result<Summary, Error> {
    let repo = ".";
    git::require_clean_tree(repo, config.git.require_clean_tree)?;
    git::require_branch(repo, config.git.require_branch.as_deref())?;

    let source_entry = config.manifest.iter()
        .find(|m| m.path == config.version.current_source)
        .ok_or_else(|| Error::Read { path: config.version.current_source.clone(), source: io::Error::new(io::ErrorKind::NotFound, "current_source manifest entry not found") })?;
    let source_content = read(&source_entry.path)?;
    let source_editor = manifest::editor_for(source_entry)
        .map_err(|e| Error::CurrentSource { path: source_entry.path.clone(), source: e })?;
    let current = source_editor.read_version(&source_content)
        .map_err(|e| Error::CurrentSource { path: source_entry.path.clone(), source: e })?;
    let next = semver_bump::bump(&current, bump_kind);

    let preflight_plan = preflight::plan(&config.preflight, skip_preflight);
    if !dry_run { preflight::run(&preflight_plan)?; }

    let mut touched = Vec::new();
    let mut paths_to_stage = Vec::new();
    for m in &config.manifest {
        let content = read(&m.path)?;
        let editor = manifest::editor_for(m).map_err(|e| Error::Manifest { path: m.path.clone(), source: e })?;
        let old = editor.read_version(&content).map_err(|e| Error::Manifest { path: m.path.clone(), source: e })?;
        let new_content = editor.write_version(&content, &next).map_err(|e| Error::Manifest { path: m.path.clone(), source: e })?;
        touched.push(Touched { path: m.path.clone(), old: old.to_string(), new: next.to_string() });
        if content != new_content {
            if !dry_run { fs::write(&m.path, new_content).map_err(|e| Error::Write { path: m.path.clone(), source: e })?; }
            paths_to_stage.push(m.path.clone());
        }
    }
    if let Some(cl_path) = &config.changelog.path {
        if !dry_run { changelog::update(cl_path, &git::tag_name(&config.git.tag_prefix, &next.to_string()), &config.changelog.entry_template)?; }
        paths_to_stage.push(cl_path.clone());
    }

    let commit_message = git::commit_message(&config.git.commit_message, &next.to_string());
    let tag = git::tag_name(&config.git.tag_prefix, &next.to_string());
    git::stage(repo, &paths_to_stage, dry_run)?;
    git::commit(repo, &commit_message, dry_run)?;
    git::tag(repo, &tag, &next.to_string(), dry_run)?;

    Ok(Summary { source: source_entry.path.clone(), current, next, dry_run, preflight: preflight_plan, touched, changelog: config.changelog.path.clone(), commit_message, tag })
}

pub fn doctor(config: &Config) -> Result<Vec<Drift>, Error> {
    let source_entry = config.manifest.iter()
        .find(|m| m.path == config.version.current_source)
        .ok_or_else(|| Error::Read { path: config.version.current_source.clone(), source: io::Error::new(io::ErrorKind::NotFound, "current_source manifest entry not found") })?;
    let source_content = read(&source_entry.path)?;
    let source_editor = manifest::editor_for(source_entry)
        .map_err(|e| Error::CurrentSource { path: source_entry.path.clone(), source: e })?;
    let expected = source_editor.read_version(&source_content)
        .map_err(|e| Error::CurrentSource { path: source_entry.path.clone(), source: e })?;

    let mut drifts = Vec::new();
    for m in &config.manifest {
        if m.path == source_entry.path { continue; }
        let content = read(&m.path)?;
        let editor = manifest::editor_for(m).map_err(|e| Error::Manifest { path: m.path.clone(), source: e })?;
        let actual = editor.read_version(&content).map_err(|e| Error::Manifest { path: m.path.clone(), source: e })?;
        if actual != expected { drifts.push(Drift { path: m.path.clone(), expected: expected.to_string(), actual: actual.to_string() }); }
    }
    Ok(drifts)
}

fn read(path: impl AsRef<Path>) -> Result<String, Error> {
    let path = path.as_ref();
    fs::read_to_string(path).map_err(|e| Error::Read { path: path.display().to_string(), source: e })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config;
    use std::fs;

    fn tmp(prefix: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("{}-{}", prefix, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(dir: &std::path::PathBuf, name: &str, text: &str) {
        fs::write(dir.join(name), text).unwrap();
    }

    fn load(dir: &std::path::PathBuf, source: &str, extra: &str) -> Config {
        let pkg = dir.join(source).to_string_lossy().to_string();
        let cargo = dir.join("Cargo.toml").to_string_lossy().to_string();
        write(dir, "release.toml", &format!(
            "[version]\ncurrent_source = \"{pkg}\"\n\n[[manifest]]\npath = \"{pkg}\"\nkind = \"json\"\nfield = \"version\"\n\n[[manifest]]\npath = \"{cargo}\"\nkind = \"cargo-package\"\n\n{extra}"
        ));
        config::load(dir.join("release.toml")).unwrap()
    }

    #[test]
    fn doctor_reports_no_drift_when_consistent() {
        let dir = tmp("cutver-doc-ok");
        write(&dir, "package.json", r#"{"version": "1.2.3"}"#);
        write(&dir, "Cargo.toml", "[package]\nversion = \"1.2.3\"\n");
        let drifts = doctor(&load(&dir, "package.json", "")).unwrap();
        assert!(drifts.is_empty());
    }

    #[test]
    fn doctor_reports_drift_when_manifests_differ() {
        let dir = tmp("cutver-doc-drift");
        write(&dir, "package.json", r#"{"version": "1.2.3"}"#);
        write(&dir, "Cargo.toml", "[package]\nversion = \"1.0.0\"\n");
        let drifts = doctor(&load(&dir, "package.json", "")).unwrap();
        assert_eq!(drifts.len(), 1);
        assert_eq!(drifts[0].path, dir.join("Cargo.toml").to_string_lossy().to_string());
        assert_eq!(drifts[0].expected, "1.2.3");
        assert_eq!(drifts[0].actual, "1.0.0");
    }

    #[test]
    fn dry_run_leaves_files_unchanged() {
        let dir = tmp("cutver-bump-dry");
        write(&dir, "package.json", r#"{"version": "1.2.3"}"#);
        write(&dir, "Cargo.toml", "[package]\nversion = \"1.2.3\"\n");
        let summary = run(&load(&dir, "package.json", "[git]\nrequire_clean_tree = false\n"), Bump::Minor, true, &[]).unwrap();
        assert_eq!(summary.next, Version::parse("1.3.0").unwrap());
        assert!(summary.dry_run);
        assert_eq!(fs::read_to_string(dir.join("Cargo.toml")).unwrap(), "[package]\nversion = \"1.2.3\"\n");
    }
}
