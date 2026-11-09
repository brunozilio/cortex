//! Work on a conversation's engine session besides prompts: measuring how
//! full its context window is after every reply, and compacting it on
//! request. Each is a short run of its own that never touches the
//! transcript.

use superfik_core::{EngineEvent, RunId, SessionCommand};
use superfik_protocol::ClientCommand;

use super::{Conversation, Effect, Notice, Session};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Upkeep {
    pub run: RunId,
    pub command: SessionCommand,
}

impl Conversation {
    pub fn is_compacting(&self) -> bool {
        self.upkeep.is_some_and(|u| u.command == SessionCommand::Compact)
    }

    pub fn is_measuring(&self) -> bool {
        self.upkeep.is_some_and(|u| u.command == SessionCommand::MeasureContext)
    }

    pub fn can_compact(&self) -> bool {
        self.engine_session.is_some() && !self.is_running() && !self.is_compacting()
    }

    pub(super) fn owns_upkeep(&self, run: RunId) -> bool {
        self.upkeep.is_some_and(|u| u.run == run)
    }

    pub(super) fn forget_session(&mut self) {
        self.engine_session = None;
        self.context = None;
        self.compaction = None;
    }

    pub(super) fn stop_measuring(&mut self) -> Vec<Effect> {
        match self.upkeep.take_if(|u| u.command == SessionCommand::MeasureContext) {
            Some(upkeep) => vec![Effect::Command(Box::new(ClientCommand::CancelRun { run: upkeep.run }))],
            None => Vec::new(),
        }
    }
}

impl Session {
    pub(super) fn compact(&mut self) -> Vec<Effect> {
        let index = self.active_index();
        if !self.conversations[index].can_compact() {
            return Vec::new();
        }
        self.conversations[index].compaction = None;
        self.start_upkeep(index, SessionCommand::Compact)
    }

    pub(super) fn start_upkeep(&mut self, index: usize, command: SessionCommand) -> Vec<Effect> {
        let Some(resume) = self.conversations.get(index).and_then(|c| c.engine_session.clone()) else {
            return Vec::new();
        };
        let Some(request) = self.session_request(index, command, String::new(), Some(resume)) else {
            return Vec::new();
        };
        let mut effects = self.conversations[index].stop_measuring();
        let run = RunId::new();
        self.conversations[index].upkeep = Some(Upkeep { run, command });
        effects.push(Effect::Command(Box::new(ClientCommand::SubmitPrompt { run, request })));
        effects
    }

    pub(super) fn upkeep_event(&mut self, index: usize, event: &EngineEvent) -> Vec<Effect> {
        let c = &mut self.conversations[index];
        let command = c.upkeep.map(|u| u.command);
        match event {
            EngineEvent::ContextMeasured(usage) => c.context = Some(usage.clone()),
            EngineEvent::Compacted(compaction) => c.compaction = Some(*compaction),
            EngineEvent::Diagnosed(problem) => self.diagnosed(index, *problem),
            EngineEvent::Failed(message) if command == Some(SessionCommand::Compact) => {
                self.notice.get_or_insert_with(|| Notice::Raw(message.clone()));
            }
            _ => {}
        }
        if !event.is_terminal() {
            return Vec::new();
        }
        self.conversations[index].upkeep = None;
        match (command, event) {
            (Some(SessionCommand::Compact), EngineEvent::Completed { .. }) => {
                self.start_upkeep(index, SessionCommand::MeasureContext)
            }
            _ => Vec::new(),
        }
    }

    pub(super) fn active_index(&self) -> usize {
        self.conversations.iter().position(|c| c.id == self.active).unwrap_or(0)
    }
}
