use std::sync::Arc;

use domain::model_roles::{ModelRole, ModelRoles};

use super::equip::{MainAgent, Toolbox};
use super::load_session::LoadedSession;
use super::policy::Governed;
use crate::agent_runtime::{self, WorktreeLease};
use crate::engine::job::Job;
use crate::engine::stop::Stop;
use crate::engine::{roles, small_model};
use crate::prompt::settings::Settings;
use crate::tools::{self, Workspace};

pub(in crate::engine) struct SessionWorktree {
    pub(in crate::engine) lease: WorktreeLease,
    pub(in crate::engine) cleanup: Workspace,
}

pub(in crate::engine) struct Runtime {
    pub(in crate::engine) workspace: Workspace,
    pub(in crate::engine) toolbox: Toolbox,
    pub(in crate::engine) main: MainAgent,
    pub(in crate::engine) worktree: SessionWorktree,
}

impl Job {
    pub(in crate::engine) fn prepare_runtime(
        &mut self,
        governed: Governed,
        settings: &Settings,
        session: &LoadedSession,
    ) -> Result<Runtime, Stop> {
        let Governed { mut workspace, mut toolbox, main, roles } = governed;
        workspace.home = crate::brief::home(self.request.folder());
        workspace.teammate = workspace.home.as_deref().and_then(crate::team::Me::from_env);
        workspace.models.fallbacks = roles::fallbacks(&self.request.session, &settings.fallbacks);
        workspace.models.small_model = self.small_model(&roles);
        workspace.models.roles = roles;
        workspace.services.network = Some(tools::Network(Arc::clone(&self.mcp)));
        workspace.services.search = self.source.searcher(&self.request).map(tools::WebSearcher);
        workspace.added_dirs =
            crate::brief::added_dirs(&self.request.session, &settings.added_dirs, workspace.home.as_deref());
        workspace.hooks.extend(&main.config.hooks);
        let worktree = self.session_worktree(&main, &mut workspace, session)?;
        self.settle_model(&main, &mut workspace)?;
        if let Some(schema) = settings.schema.clone() {
            let structured = tools::structured::Structured::new(schema).map_err(Stop::failed)?;
            toolbox.tools.push(Box::new(structured));
        }
        if let Some(judge) = &workspace.services.judge {
            judge.settle(tools::host_judge::Grounds::of(&workspace));
        }
        Ok(Runtime { workspace, toolbox, main, worktree })
    }

    fn small_model(&self, roles: &ModelRoles) -> Option<String> {
        let small = small_model::pick(
            self.request.session.model.provider,
            &self.request.session.model.id,
            roles.get(ModelRole::Small),
            &self.source.catalog(&self.request),
        );
        if let Some(why) = &small.ignored {
            tracing::warn!("{why}");
        }
        small.model
    }

    fn session_worktree(
        &self,
        main: &MainAgent,
        workspace: &mut Workspace,
        session: &LoadedSession,
    ) -> Result<SessionWorktree, Stop> {
        let previous = self.sessions.load_worktree(&self.request.session.cwd, &session.id).map_err(Stop::failed)?;
        let resumed = previous.is_some();
        let setup = match previous {
            Some(state) => agent_runtime::resume(&main.config, main.name(), &state, workspace),
            None => agent_runtime::prepare(&main.config, main.name(), workspace),
        }
        .map_err(Stop::failed)?;
        workspace.cwd.clone_from(&setup.cwd);
        workspace.worktree = setup.worktree_state();
        if let Some(context) = &setup.context {
            workspace.rules.clone_from(&context.rules);
        }
        let cleanup = workspace.fork(Arc::default());
        let mut lease = WorktreeLease::new(setup);
        if resumed {
            lease.retain();
        }
        self.keep_worktree(&mut lease, session)?;
        Ok(SessionWorktree { lease, cleanup })
    }

    fn keep_worktree(&self, lease: &mut WorktreeLease, session: &LoadedSession) -> Result<(), Stop> {
        let Some(state) = lease.setup.worktree_state() else { return Ok(()) };
        self.recorder.iter().for_each(|recorder| recorder.announce(&self.gate));
        let stored = self
            .sessions
            .save_for(&self.request.session.cwd, &session.id, &session.history)
            .and_then(|()| self.sessions.save_worktree(&self.request.session.cwd, &session.id, &state));
        if let Err(error) = stored {
            return Err(Stop::failed(match lease.finish() {
                Ok(Some(path)) => format!("{error}; worktree retained at {}", path.display()),
                Err(cleanup) => format!("{error}; worktree cleanup failed: {cleanup}"),
                _ => error,
            }));
        }
        lease.retain();
        Ok(())
    }

    fn settle_model(&mut self, main: &MainAgent, workspace: &mut Workspace) -> Result<(), Stop> {
        self.request.session.effort = main.config.effort.unwrap_or(self.request.session.effort);
        let asked = &self.request.session.model;
        let chosen = match &main.config.model {
            Some(_) => main.config.model(&asked.id, asked.provider).map_err(Stop::failed)?,
            None => workspace.models.roles.for_turn(&asked.id, self.request.policy.permission).to_owned(),
        };
        if chosen != self.request.session.model.id {
            self.request.session.model.id.clone_from(&chosen);
            workspace.models.switch_to(chosen);
        }
        Ok(())
    }
}
