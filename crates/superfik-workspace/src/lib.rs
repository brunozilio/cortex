//! Superfik workspace inspection.
//!
//! IO lives behind traits so callers (the backend) can swap them for
//! fakes: [`GitInspector`] describes the repository, [`BranchSwitcher`]
//! checks out branches and [`GitHubClient`] optionally enriches the
//! repository through `gh`. [`snapshot`] describes a directory as a
//! [`WorkspaceSnapshot`].

mod error;
pub mod executable;
pub mod git;
pub mod github;
mod snapshot;

pub use error::WorkspaceError;
pub use git::{BranchPusher, BranchSwitcher, GitCli, GitInspector, GitRunner, SystemGit, Worktrees, remote_slug};
pub use github::{GhCli, GitHubClient, NoGitHub, clone_destination, enrich};
pub use snapshot::snapshot;
pub use superfik_core::WorkspaceSnapshot;
