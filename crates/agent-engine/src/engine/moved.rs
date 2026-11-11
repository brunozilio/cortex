//! The session moving mid-run (`EnterWorktree`, `ExitWorktree`): the
//! workspace, its project instructions and the system prompt follow the new
//! folder, and the session store remembers where later runs go on.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use domain::{PromptRequest, SessionId};

use crate::agent_runtime::{Guard, Relocation, Setup};
use crate::sessions::Sessions;
use crate::tools::Workspace;

pub(super) struct Move<'a> {
    pub(super) request: &'a PromptRequest,
    pub(super) sessions: &'a dyn Sessions,
    pub(super) session: &'a SessionId,
    /// What the run's system prompt adds after the environment's part.
    pub(super) extra: &'a str,
}

impl Move<'_> {
    /// Moves as `relocation` says: the new system prompt and folder, and
    /// what to tell the user when the worktree left could not go. A
    /// worktree just made is removed again when the move fails.
    pub(super) fn to(
        &self,
        relocation: Relocation,
        workspace: Option<&mut Workspace>,
        guard: &mut Guard,
    ) -> Result<Moved, String> {
        let Relocation { setup, remove } = relocation;
        let entered = setup.worktree_root.is_some().then(|| setup.clone());
        let moved = match workspace {
            Some(workspace) => self.switch(setup, workspace, guard),
            None => Err("the workspace is shared".into()),
        };
        if moved.is_err()
            && let Some(entered) = entered
        {
            let _ = crate::agent_runtime::cleanup(&entered);
        }
        let (system, cwd) = moved?;
        let kept = remove.and_then(|(state, discard)| {
            crate::agent_runtime::remove(&state, discard, &AtomicBool::new(false))
                .err()
                .map(|error| format!("The worktree at {} was kept: {error}", state.cwd.display()))
        });
        Ok(Moved { system, cwd, kept })
    }

    /// The fallible reads first; the session store and the workspace
    /// change only once nothing else can fail.
    fn switch(&self, setup: Setup, workspace: &mut Workspace, guard: &mut Guard) -> Result<(String, PathBuf), String> {
        let brief = crate::brief::Brief::load(&PromptRequest { cwd: setup.cwd.clone(), ..self.request.clone() })?;
        let Some(mut context) = workspace.prompt_context.clone() else {
            return Err("the run has no environment to move".into());
        };
        let state = setup.worktree_state();
        match &state {
            Some(state) => self.sessions.save_worktree(self.request, self.session, state)?,
            None => self.sessions.forget_worktree(self.request, self.session)?,
        }
        let cwd = setup.cwd.clone();
        context.cwd = cwd.clone();
        context.git = brief.git;
        context.instructions = brief.instructions;
        let system = format!("{}{}", crate::prompt::system(&context), self.extra);
        workspace.cwd = cwd.clone();
        workspace.worktree = state;
        workspace.rules = brief.rules;
        workspace.prompt_context = Some(context);
        guard.moved(setup);
        Ok((system, cwd))
    }
}

/// Where the session went.
pub(super) struct Moved {
    pub(super) system: String,
    pub(super) cwd: PathBuf,
    /// Why the worktree left was not removed, when it was not.
    pub(super) kept: Option<String>,
}
