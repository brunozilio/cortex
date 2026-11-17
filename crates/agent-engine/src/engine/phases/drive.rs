use domain::EngineEvent;

use super::open::Opened;
use crate::agent::{Agent, Ending};
use crate::engine::job::Job;
use crate::engine::moved::Move;
use crate::model::ModelError;
use crate::tools::subagent::FoundAgent;
use crate::transcript::{Role, Usage};

impl Job {
    pub(in crate::engine) fn drive(&self, opened: &mut Opened, agents: &[FoundAgent]) -> Result<Usage, Ending> {
        let emit = |event: EngineEvent| {
            self.gate.emit(event);
        };
        loop {
            let agent = Agent {
                api: &opened.api,
                tools: &opened.toolbox.tools,
                workspace: &opened.workspace,
                control: &self.control,
                emit: &emit,
                request: opened.template.clone(),
                agents,
                depth: 0,
                turn_limit: opened.turn_limit,
            };
            let before = opened.history.len();
            let outcome = agent.run_keeping(&mut opened.history, self.kept_at_every_transition());
            drop(agent);
            let said = &opened.history[before.min(opened.history.len())..];
            let used = said.iter().filter(|message| message.role == Role::Assistant).count();
            opened.turn_limit = opened.turn_limit.saturating_sub(used);
            let Some(relocation) = self.control.take_move() else { return outcome };
            let mover = Move {
                request: &self.request,
                sessions: &*self.sessions,
                session: &opened.session,
                extra: &opened.extra,
            };
            match mover.to(relocation, &mut opened.workspace, &mut opened.worktree.lease) {
                Ok(moved) => {
                    opened.template.system = moved.system;
                    emit(EngineEvent::Moved(moved.cwd));
                    if let Some(kept) = moved.kept {
                        emit(EngineEvent::Notice(kept));
                    }
                }
                Err(error) => return Err(Ending::Failed(ModelError::Provider(error))),
            }
            match outcome {
                Err(Ending::Moved) if opened.turn_limit == 0 => return Err(Ending::TooManyTurns),
                Err(Ending::Moved) => {}
                other => return other,
            }
        }
    }
}
