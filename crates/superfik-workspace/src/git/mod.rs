//! Git repository facts, branch switching and pushing: the [`GitInspector`],
//! [`BranchSwitcher`] and [`BranchPusher`] boundaries and their `git` CLI
//! implementation.

mod cli;
mod push;
mod runner;
mod slug;
mod switch;
mod worktree;

use std::path::Path;

use superfik_core::Repository;

use crate::error::WorkspaceError;

pub use cli::GitCli;
pub use push::BranchPusher;
pub use runner::{GitRunner, SystemGit};
pub use slug::remote_slug;
pub use switch::BranchSwitcher;
pub use worktree::Worktrees;

pub trait GitInspector: Send + Sync {
    fn inspect(&self, root: &Path) -> Result<Repository, WorkspaceError>;
}
