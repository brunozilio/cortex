use super::Transcript;
use crate::{AgentAsk, EngineEvent, FailureKind};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    #[default]
    Idle,
    Running,
    Thinking,
    NeedsYou,
    Done,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pending {
    pub ask: AgentAsk,
    pub agent: Option<usize>,
}

impl Transcript {
    pub fn status(&self) -> SessionStatus {
        self.status
    }

    pub fn ask(&self) -> Option<&Pending> {
        self.ask.as_ref()
    }

    pub fn failure(&self) -> Option<FailureKind> {
        self.failure
    }

    pub(super) fn started_run(&mut self) {
        self.status = SessionStatus::Running;
        self.failure = None;
        self.ask = None;
        self.asking.clear();
    }

    pub(super) fn observe(&mut self, event: &EngineEvent) {
        match event {
            EngineEvent::AgentEvent { index, event } => {
                if let EngineEvent::Asked(ask) = event.as_ref() {
                    self.asking.insert(ask.id.clone(), *index);
                }
            }
            EngineEvent::Asked(ask) => {
                let agent = self.asking.remove(&ask.id);
                self.ask = Some(Pending { ask: ask.clone(), agent });
                self.status = SessionStatus::NeedsYou;
            }
            EngineEvent::AskWithdrawn(id) | EngineEvent::Answered { ask: id, .. } => {
                if self.ask.as_ref().is_some_and(|pending| pending.ask.id == *id) {
                    self.ask = None;
                    self.status = SessionStatus::Running;
                }
            }
            EngineEvent::Failure(kind) => self.failure = Some(*kind),
            EngineEvent::Completed { .. } => self.ended(SessionStatus::Done),
            EngineEvent::Failed(_) => self.ended(SessionStatus::Failed),
            EngineEvent::Cancelled => self.ended(SessionStatus::Cancelled),
            EngineEvent::ThinkingDelta(_) if self.ask.is_none() => self.status = SessionStatus::Thinking,
            EngineEvent::TextDelta(_)
            | EngineEvent::ToolStarted(_)
            | EngineEvent::ToolInputDelta { .. }
            | EngineEvent::AgentSpawned { .. }
                if self.ask.is_none() =>
            {
                self.status = SessionStatus::Running;
            }
            _ => {}
        }
    }

    fn ended(&mut self, status: SessionStatus) {
        self.status = status;
        self.ask = None;
        self.asking.clear();
    }
}
