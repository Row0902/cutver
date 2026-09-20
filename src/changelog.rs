use crate::atomic;
use std::io;
use thiserror::Error;

mod extract;
mod render;
mod update;

pub use extract::{extract_latest, extract_version, list_versions, read_latest, read_version};
pub use render::render_body;
pub use update::{format_date, update};

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to read changelog '{path}': {source}")]
    Read {
        path: String,
        #[source]
        source: io::Error,
    },
    #[error("failed to write changelog '{path}': {source}")]
    Write {
        path: String,
        #[source]
        source: atomic::Error,
    },
    #[error("no release section found in changelog '{path}'")]
    NoReleaseSection { path: String },
    #[error("version '{version}' not found in changelog '{path}'")]
    VersionNotFound { version: String, path: String },
}
