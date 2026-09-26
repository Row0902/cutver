use crate::atomic;
use std::io;
use thiserror::Error;

mod context;
mod extract;
mod render;
mod update;

pub use context::{
    CommitContext, ReleaseContext, build_context, build_context_auto, build_context_with_filter, filter_commits,
};
pub use extract::{extract_latest, extract_version, list_versions, read_latest, read_version};
pub use render::{render_body, render_body_with_context, render_template};
pub use update::{format_date, update};

pub type ChangelogError = Error;

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
    #[error("failed to render changelog template: {detail}")]
    TemplateRender { detail: String },
    #[error("failed to read template file '{path}': {source}")]
    TemplateFileRead {
        path: String,
        #[source]
        source: io::Error,
    },
}
