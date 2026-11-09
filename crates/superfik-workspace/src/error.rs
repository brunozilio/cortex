//! Errors raised while inspecting a workspace.

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    #[error("{0} is not inside a git repository")]
    NotARepository(PathBuf),
    #[error("`git {command}` failed: {message}")]
    Git { command: String, message: String },
    #[error("`{0}` is not a branch name")]
    InvalidBranch(String),
    #[error("GitHub is not available (install `gh` and run `gh auth login`) to clone {0}")]
    GitHubUnavailable(String),
    #[error("`gh repo clone {slug}` failed")]
    Clone { slug: String },
    #[error("`gh {command}` failed")]
    Gh { command: String },
    #[error("cannot read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}
