use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use domain::{PromptRequest, SessionId};

use crate::agent_runtime::{Relocation, Setup, WorktreeLease};
use crate::sessions::Sessions;
use crate::tools::Workspace;

pub(super) struct Move<'a> {
    pub(super) request: &'a PromptRequest,
    pub(super) sessions: &'a dyn Sessions,
    pub(super) session: &'a SessionId,
    pub(super) extra: &'a str,
}

impl Move<'_> {
    pub(super) fn to(
        &self,
        relocation: Relocation,
        workspace: &mut Workspace,
        lease: &mut WorktreeLease,
    ) -> Result<Moved, String> {
        let Relocation { setup, remove } = relocation;
        let entered = setup.worktree_root.is_some().then(|| setup.clone());
        let moved = self.switch(setup, workspace, lease);
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

    fn switch(
        &self,
        setup: Setup,
        workspace: &mut Workspace,
        lease: &mut WorktreeLease,
    ) -> Result<(String, PathBuf), String> {
        let folder = domain::Folder { cwd: &setup.cwd, extensions: self.request.host.extensions.as_ref() };
        let brief = crate::brief::Brief::load(folder)?;
        let Some(mut context) = workspace.prompt_context.clone() else {
            return Err("the run has no environment to move".into());
        };
        let state = setup.worktree_state();
        match &state {
            Some(state) => self.sessions.save_worktree(&self.request.session.cwd, self.session, state)?,
            None => self.sessions.forget_worktree(&self.request.session.cwd, self.session)?,
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
        lease.moved(setup);
        Ok((system, cwd))
    }
}

pub(super) struct Moved {
    pub(super) system: String,
    pub(super) cwd: PathBuf,
    pub(super) kept: Option<String>,
}
