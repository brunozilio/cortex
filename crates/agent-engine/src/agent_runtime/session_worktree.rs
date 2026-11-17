use std::sync::atomic::AtomicBool;

use super::git::output;
use super::{Setup, WorktreeState, managed, prepare_as};
use crate::tools::Workspace;
use crate::tools::subagent_config::AgentConfig;

#[derive(Debug)]
pub struct Entered {
    pub setup: Setup,
    pub commit: String,
    pub subject: String,
    pub left_behind: usize,
}

pub fn enter(workspace: &Workspace) -> Result<Entered, String> {
    let config = AgentConfig { worktree: true, ..Default::default() };
    let setup = prepare_as(&config, "main", workspace)?;
    let Some(root) = setup.worktree_root.clone() else {
        return Ok(Entered { setup, commit: String::new(), subject: String::new(), left_behind: 0 });
    };
    let stop = &workspace.stop;
    let described = output(&setup.cwd, &["log".as_ref(), "-1".as_ref(), "--format=%h %s".as_ref()], stop)?;
    let (commit, subject) = described.trim().split_once(' ').unwrap_or((described.trim(), ""));
    let status = output(&root, &["status".as_ref(), "--porcelain".as_ref(), "--untracked-files=all".as_ref()], stop)?;
    let worktrees = concat!(brand::dir!(), "/worktrees/");
    let left_behind = status.lines().filter(|line| line.len() > 3 && !line[3..].starts_with(worktrees)).count();
    Ok(Entered { commit: commit.to_owned(), subject: subject.to_owned(), left_behind, setup })
}

#[derive(Debug)]
pub struct Relocation {
    pub setup: Setup,
    pub remove: Option<(WorktreeState, bool)>,
}

pub fn remove(state: &WorktreeState, discard: bool, stop: &AtomicBool) -> Result<(), String> {
    removable(state, discard, stop)?;
    output(&state.root, &["worktree".as_ref(), "remove".as_ref(), "--force".as_ref(), state.cwd.as_os_str()], stop)
        .map(|_| ())
}

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
