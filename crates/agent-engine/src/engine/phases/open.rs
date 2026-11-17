use std::sync::Arc;

use domain::{EngineEvent, SessionCommand, SessionId};

use super::equip::Toolbox;
use super::load_session::LoadedSession;
use super::runtime::{Runtime, SessionWorktree};
use super::system_prompt::Framed;
use super::user_message::Opening;
use crate::brief::Brief;
use crate::checkpoints::Checkpoint;
use crate::engine::job::Job;
use crate::engine::stop::Stop;
use crate::meter::{Ledger, Metered};
use crate::model::{ModelApi, TurnRequest};
use crate::tools::Workspace;
use crate::tools::subagent_config::AgentConfig;
use crate::transcript::Message;
use crate::upkeep;

pub(in crate::engine) struct Opened {
    pub(in crate::engine) workspace: Workspace,
    pub(in crate::engine) toolbox: Toolbox,
    pub(in crate::engine) worktree: SessionWorktree,
    pub(in crate::engine) api: Metered,
    pub(in crate::engine) template: TurnRequest,
    pub(in crate::engine) extra: String,
    pub(in crate::engine) session: SessionId,
    pub(in crate::engine) history: Vec<Message>,
    pub(in crate::engine) checkpoint: Checkpoint,
    pub(in crate::engine) turn_limit: usize,
}

impl Job {
    pub(in crate::engine) fn open(
        &mut self,
        runtime: Runtime,
        brief: &Brief,
        session: LoadedSession,
    ) -> Result<Opened, Stop> {
        let LoadedSession { id: session, mut history } = session;
        if self.request.session.command == Some(SessionCommand::MeasureContext) {
            let framed = self.frame(runtime, brief, &session)?;
            return Err(self.measured(&framed.template, &history));
        }
        let api = self.signed_in()?;
        let framed = self.frame(runtime, brief, &session)?;
        let Framed { mut workspace, toolbox, main, worktree, mut template, extra } = framed;
        let api = self.metered_for_the_run(api, &mut template, &mut workspace);
        if self.request.session.command == Some(SessionCommand::Compact) {
            return Err(self.compacted(&api, &template, &workspace, &session, &mut history));
        }
        let turn_limit = self.turn_limit(&main.config);
        let user = self.user_message(&main.config, &brief.skills, &workspace, history.is_empty())?;
        let opening = Opening { api: &api, template: &template, workspace: &workspace, session: &session };
        let user = self.submitted(user, &opening)?;
        let checkpoint = self.open_checkpoint(&session, &history, &workspace);
        crate::transcript::prompted(&mut history, user);
        Ok(Opened { workspace, toolbox, worktree, api, template, extra, session, history, checkpoint, turn_limit })
    }

    fn measured(&self, template: &TurnRequest, history: &[Message]) -> Stop {
        let usage = upkeep::measure(&self.request.session.model, &template.system, &template.tools, history);
        self.gate.emit(EngineEvent::ContextMeasured(usage));
        Stop::completed(Some(0.0))
    }

    fn metered_for_the_run(
        &mut self,
        api: Arc<dyn ModelApi>,
        template: &mut TurnRequest,
        workspace: &mut Workspace,
    ) -> Metered {
        let ledger = Arc::new(Ledger::default());
        if self.request.session.auto_pick && self.request.session.command.is_none() {
            self.auto_pick(&api, &ledger, template, &mut workspace.models);
        }
        self.metered(api, &ledger, self.request.session.model.clone())
    }

    fn compacted(
        &self,
        api: &Metered,
        template: &TurnRequest,
        workspace: &Workspace,
        session: &SessionId,
        history: &mut Vec<Message>,
    ) -> Stop {
        let focus = Some(self.request.prompt.text.as_str()).filter(|focus| !focus.trim().is_empty());
        match upkeep::compact_hooked_on(api, template, history, workspace, "manual", focus) {
            Ok(compaction) => {
                if let Err(error) = self.save(session, history) {
                    return Stop::failed(format!("Could not save the compacted conversation: {error}"));
                }
                self.gate.emit(EngineEvent::Compacted(compaction));
                Stop::completed(api.ledger.cost())
            }
            Err(error) => Stop::Model(error),
        }
    }

    fn turn_limit(&self, config: &AgentConfig) -> usize {
        self.request
            .policy
            .max_turns
            .map(|turns| turns as usize)
            .or(config.max_turns)
            .unwrap_or(crate::agent::MAX_TURNS)
    }
}
