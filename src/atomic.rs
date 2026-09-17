use std::fs;
use std::io;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid target path '{path}'")]
    InvalidPath { path: String },
    #[error("failed to write temp file '{path}': {source}")]
    TempWrite { path: String, source: io::Error },
    #[error("failed to rename '{src}' to '{dst}': {source}")]
    Rename { src: String, dst: String, source: io::Error },
}

/// Write `contents` to `path` atomically using a same-directory temp file and
/// a final `rename`. No `fsync` is performed; the caller is assumed to value
/// speed over power-loss durability for a release-script tool.
pub fn write_atomic(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> Result<(), Error> {
    let path = path.as_ref();
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .ok_or_else(|| Error::InvalidPath { path: path.display().to_string() })?
        .to_string_lossy();
    let tmp = dir.join(format!(".{name}.cutver-tmp"));

    fs::write(&tmp, contents).map_err(|e| Error::TempWrite { path: tmp.display().to_string(), source: e })?;
    fs::rename(&tmp, path).map_err(|e| Error::Rename {
        src: tmp.display().to_string(),
        dst: path.display().to_string(),
        source: e,
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp_dir(prefix: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("{prefix}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    #[cfg_attr(not(unix), ignore)]
    fn creates_new_file_and_cleans_temp() {
        let dir = tmp_dir("cutver-atomic-new");
        let path = dir.join("out.txt");
        write_atomic(&path, "hello").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "hello");
        assert!(!dir.join(".out.txt.cutver-tmp").exists());
    }

    #[test]
    #[cfg_attr(not(unix), ignore)]
    fn replaces_existing_file_atomically() {
        let dir = tmp_dir("cutver-atomic-replace");
        let path = dir.join("out.txt");
        fs::write(&path, "old").unwrap();
        write_atomic(&path, "new").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "new");
    }

    #[test]
    #[cfg_attr(not(unix), ignore)]
    fn rename_to_directory_fails_cleanly() {
        let dir = tmp_dir("cutver-atomic-dir");
        let target = dir.join("blocked");
        fs::create_dir(&target).unwrap();
        assert!(write_atomic(&target, "x").is_err());
    }
}
