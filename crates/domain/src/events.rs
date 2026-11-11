use serde::{Deserialize, Serialize};

use crate::agents::AgentStatus;
use crate::approval::{AgentAsk, AskId};
use crate::engines::EngineProblem;
use crate::failure::FailureKind;
use crate::ids::{Id, SessionId};
use crate::usage::{Compaction, ContextUsage};

#[derive(Debug)]
pub enum RunMarker {}
pub type RunId = Id<RunMarker>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub summary: String,
    #[serde(default)]
    pub output: Option<String>,
    #[serde(default)]
    pub failed: bool,
}

impl ToolCall {
    pub fn new(id: impl Into<String>, name: impl Into<String>, summary: impl Into<String>) -> Self {
        Self { id: id.into(), name: name.into(), summary: summary.into(), output: None, failed: false }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EngineEvent {
    SessionStarted(SessionId),
    Usage(crate::billing::ModelUsage),
    StructuredOutput(serde_json::Value),
    TextDelta(String),
    ThinkingDelta(String),
    ToolInputDelta {
        name: String,
        piece: String,
    },
    ToolStarted(ToolCall),
    ToolLog {
        id: String,
        text: String,
    },
    ToolFinished {
        id: String,
    },
    ToolResult {
        id: String,
        output: String,
        failed: bool,
    },
    AgentSpawned {
        index: usize,
        kind: String,
        summary: String,
        #[serde(default)]
        prompt: String,
    },
    AgentEvent {
        index: usize,
        event: Box<EngineEvent>,
    },
    AgentUpdate {
        index: usize,
        status: AgentStatus,
        summary: Option<String>,
    },
    Diagnosed(EngineProblem),
    ContextMeasured(ContextUsage),
    Compacted(Compaction),
    Chore(crate::usage::ChoreAnswer),
    Checkpoint(u32),
    Rewound(Rewound),
    Asked(AgentAsk),
    AskWithdrawn(AskId),
    Steered(String),
    FellBack(String),
    Notified(Vec<crate::inbox::Notice>),
    Moved(std::path::PathBuf),
    Tasks(Vec<TaskItem>),
    Unsteered(Vec<String>),
    ModeChanged(crate::PermissionMode),
    Verified(crate::verification::Verdict),
    Completed {
        duration_ms: u64,
        cost_usd: Option<f64>,
    },
    Failure(FailureKind),
    Failed(String),
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskItem {
    pub id: String,
    pub subject: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rewound {
    pub checkpoint: u32,
    pub prompt: String,
    pub restored: Vec<std::path::PathBuf>,
    pub conversation: bool,
}

impl EngineEvent {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed { .. } | Self::Failed(_) | Self::Cancelled)
    }

    pub fn is_partial_input(&self) -> bool {
        match self {
            Self::ToolInputDelta { .. } => true,
            Self::AgentEvent { event, .. } => event.is_partial_input(),
            _ => false,
        }
    }

    pub fn absorb(&mut self, next: &EngineEvent) -> bool {
        match (self, next) {
            (Self::TextDelta(text), Self::TextDelta(more)) | (Self::ThinkingDelta(text), Self::ThinkingDelta(more)) => {
                text.push_str(more);
                true
            }
            (Self::AgentEvent { index, event }, Self::AgentEvent { index: next_index, event: next_event })
                if index == next_index =>
            {
                event.absorb(next_event)
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::EngineEvent;

    fn agent(index: usize, event: EngineEvent) -> EngineEvent {
        EngineEvent::AgentEvent { index, event: Box::new(event) }
    }

    #[test]
    fn streamed_pieces_of_the_same_kind_and_agent_join_into_one() {
        let mut text = EngineEvent::TextDelta("Hel".into());
        assert!(text.absorb(&EngineEvent::TextDelta("lo".into())));
        assert_eq!(text, EngineEvent::TextDelta("Hello".into()));

        let mut nested = agent(1, EngineEvent::ThinkingDelta("a".into()));
        assert!(nested.absorb(&agent(1, EngineEvent::ThinkingDelta("b".into()))));
        assert_eq!(nested, agent(1, EngineEvent::ThinkingDelta("ab".into())));

        assert!(!nested.absorb(&agent(2, EngineEvent::ThinkingDelta("c".into()))));
        assert!(!text.absorb(&EngineEvent::ThinkingDelta("x".into())));
        assert!(!text.absorb(&EngineEvent::Cancelled));
        assert_eq!(text, EngineEvent::TextDelta("Hello".into()));
    }

    #[test]
    fn partial_tool_input_is_recognised_inside_agents_too() {
        let piece = EngineEvent::ToolInputDelta { name: "Read".into(), piece: "{".into() };
        assert!(piece.is_partial_input());
        assert!(agent(0, piece).is_partial_input());
        assert!(!EngineEvent::TextDelta("x".into()).is_partial_input());
    }
}
