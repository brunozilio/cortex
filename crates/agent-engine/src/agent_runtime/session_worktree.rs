//! The session's own worktree (`EnterWorktree`, `ExitWorktree`): made like
//! an isolated agent's, and removed on the way out only when nothing in it
//! would be lost, or the agent says to discard it.

use std::sync::atomic::AtomicBool;

use super::git::output;
use super::{Setup, WorktreeState, managed, owner, prepare_as};
use crate::tools::Workspace;
use crate::tools::subagent_config::AgentConfig;

/// A fresh worktree of the project's base for the session to work in.
pub fn enter(workspace: &Workspace) -> Result<Setup, String> {
    let config = AgentConfig { worktree: true, ..Default::default() };
    prepare_as(&config, "main", &owner(workspace)?, workspace)
}

/// Where the session moves once the turn's calls are done, and the
/// worktree it leaves, to remove then (with whether to discard changes).
#[derive(Debug)]
pub struct Relocation {
    pub setup: Setup,
    pub remove: Option<(WorktreeState, bool)>,
}

/// Removes the session's worktree, checking again that nothing would be
/// lost.
pub fn remove(state: &WorktreeState, discard: bool, stop: &AtomicBool) -> Result<(), String> {
    removable(state, discard, stop)?;
    output(&state.root, &["worktree".as_ref(), "remove".as_ref(), "--force".as_ref(), state.cwd.as_os_str()], stop)
        .map(|_| ())
}

/// Whether the session's worktree may go: refused while it holds
/// uncommitted files or commits of its own, unless `discard`.
pub fn removable(state: &WorktreeState, discard: bool, stop: &AtomicBool) -> Result<(), String> {
    if !managed(&state.root, &state.cwd) {
        return Err(concat!("not a worktree ", brand::name!(), " made").into());
    }
    if !discard {
        let head = output(&state.cwd, &["rev-parse".as_ref(), "HEAD".as_ref()], stop)?;
        let status =
            output(&state.cwd, &["status".as_ref(), "--porcelain".as_ref(), "--untracked-files=all".as_ref()], stop)?;
        if head.trim() != state.base_commit || !status.trim().is_empty() {
            return Err(format!(
                "The worktree at {} has changes (uncommitted files or commits of its own). Pass \
                 discard_changes: true to remove it anyway, or use action \"keep\".",
                state.cwd.display()
            ));
        }
    }
    Ok(())
}
