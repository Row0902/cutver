use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use thiserror::Error;

static ATOMIC_SEQ: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid target path '{path}'")]
    InvalidPath { path: String },
    #[error("failed to write temp file '{path}': {source}")]
    TempWrite { path: String, source: io::Error },
    #[error("failed to rename '{src}' to '{dst}': {source}")]
    Rename {
        src: String,
        dst: String,
        source: io::Error,
    },
}

struct TempFileGuard<'a> {
    path: &'a Path,
    active: bool,
}

impl<'a> TempFileGuard<'a> {
    fn new(path: &'a Path) -> Self {
        Self {
            path,
            active: false,
        }
    }

    fn arm(&mut self) {
        self.active = true;
    }

    fn disarm(&mut self) {
        self.active = false;
    }
}

impl Drop for TempFileGuard<'_> {
    fn drop(&mut self) {
        if self.active {
            let _ = fs::remove_file(self.path);
        }
    }
}

fn temp_file_path(path: &Path) -> Result<PathBuf, Error> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .ok_or_else(|| Error::InvalidPath {
            path: path.display().to_string(),
        })?
        .to_string_lossy();
    let pid = std::process::id();
    let seq = ATOMIC_SEQ.fetch_add(1, Ordering::Relaxed);
    Ok(dir.join(format!(".{name}.cutver-tmp-{pid}-{seq}")))
}

/// Write `contents` to `path` atomically using a unique same-directory temp file,
/// `sync_all`, and a final `rename`. Cleans up the temporary file on failure.
pub fn write_atomic(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> Result<(), Error> {
    write_atomic_impl(path.as_ref(), contents.as_ref(), |file, data| {
        file.write_all(data)?;
        file.sync_all()
    })
}

fn write_atomic_impl<F>(
    path: &Path,
    contents: &[u8],
    write_and_sync: F,
) -> Result<(), Error>
where
    F: FnOnce(&mut fs::File, &[u8]) -> io::Result<()>,
{
    let tmp = temp_file_path(path)?;
    let mut guard = TempFileGuard::new(&tmp);

    {
        let mut file = fs::File::create(&tmp).map_err(|e| Error::TempWrite {
            path: tmp.display().to_string(),
            source: e,
        })?;
        guard.arm();

        write_and_sync(&mut file, contents).map_err(|e| Error::TempWrite {
            path: tmp.display().to_string(),
            source: e,
        })?;
    }

    fs::rename(&tmp, path).map_err(|e| Error::Rename {
        src: tmp.display().to_string(),
        dst: path.display().to_string(),
        source: e,
    })?;

    guard.disarm();
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
    use std::fs;

    fn tmp_dir(prefix: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(tmp_id(prefix));
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
        let entries: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(entries, vec!["out.txt"]);
    }

    #[test]
    #[cfg_attr(not(unix), ignore)]
    fn replaces_existing_file_atomically() {
        let dir = tmp_dir("cutver-atomic-replace");
        let path = dir.join("out.txt");
        fs::write(&path, "old").unwrap();
        write_atomic(&path, "new").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "new");
        let entries: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(entries, vec!["out.txt"]);
    }

    #[test]
    #[cfg_attr(not(unix), ignore)]
    fn writes_empty_content() {
        let dir = tmp_dir("cutver-atomic-empty");
        let path = dir.join("empty.txt");
        write_atomic(&path, "").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "");
        let entries: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(entries, vec!["empty.txt"]);
    }

    #[test]
    fn temp_files_have_unique_names() {
        let path = Path::new("test.txt");
        let t1 = temp_file_path(path).unwrap();
        let t2 = temp_file_path(path).unwrap();
        assert_ne!(t1, t2);

        let t1_name = t1.file_name().unwrap().to_string_lossy();
        let pid = std::process::id();
        assert!(
            t1_name.starts_with(&format!(".test.txt.cutver-tmp-{pid}-")),
            "unexpected temp file name: {t1_name}"
        );
    }

    #[test]
    fn concurrent_writes_to_same_file_do_not_collide() {
        let dir = tmp_dir("cutver-atomic-concurrent");
        let path = dir.join("out.txt");

        let handles: Vec<_> = (0..20)
            .map(|i| {
                let path = path.clone();
                std::thread::spawn(move || {
                    write_atomic(&path, format!("content-{}", i)).unwrap();
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        let final_content = fs::read_to_string(&path).unwrap();
        assert!(
            final_content.starts_with("content-"),
            "unexpected content: {final_content}"
        );

        let entries: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(entries, vec!["out.txt"]);
    }

    #[test]
    #[cfg_attr(not(unix), ignore)]
    fn rename_to_directory_fails_cleanly_and_removes_temp() {
        let dir = tmp_dir("cutver-atomic-dir");
        let target = dir.join("blocked");
        fs::create_dir(&target).unwrap();

        let err = write_atomic(&target, "x").unwrap_err();
        match err {
            Error::Rename { .. } => {}
            other => panic!("expected Rename error, got: {other:?}"),
        }

        let entries: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(entries, vec!["blocked"]);
    }

    #[test]
    fn write_error_cleans_up_temp_file() {
        let dir = tmp_dir("cutver-atomic-write-err");
        let path = dir.join("out.txt");

        let err = write_atomic_impl(&path, b"content", |_file, _data| {
            Err(io::Error::other("simulated write failure"))
        })
        .unwrap_err();

        match err {
            Error::TempWrite { .. } => {}
            other => panic!("expected TempWrite error, got: {other:?}"),
        }

        let entries: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        assert!(
            entries.is_empty(),
            "expected empty directory after failed write, found: {entries:?}"
        );
    }

    #[test]
    fn invalid_path_fails() {
        let err = write_atomic(Path::new("/"), "content").unwrap_err();
        match err {
            Error::InvalidPath { .. } => {}
            other => panic!("expected InvalidPath error, got: {other:?}"),
        }
    }

    #[test]
    fn nonexistent_parent_dir_fails_cleanly() {
        let dir = tmp_dir("cutver-atomic-nonexistent");
        let target = dir.join("no_such_subdir").join("out.txt");
        let err = write_atomic(&target, "data").unwrap_err();
        match err {
            Error::TempWrite { .. } => {}
            other => panic!("expected TempWrite error, got: {other:?}"),
        }
    }
}
