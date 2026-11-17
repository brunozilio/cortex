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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<serde_json::Value>,
}

impl ToolCall {
    pub fn new(id: impl Into<String>, name: impl Into<String>, summary: impl Into<String>) -> Self {
        Self { id: id.into(), name: name.into(), summary: summary.into(), output: None, failed: false, input: None }
    }

    pub fn with_input(self, input: serde_json::Value) -> Self {
        Self { input: Some(input), ..self }
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
    ToolProgress {
        id: String,
        line: String,
    },
    ToolFinished {
        id: String,
    },
    ToolResult {
        id: String,
        output: String,
        failed: bool,
    },
    BackgroundStarted {
        id: String,
    },
    BackgroundEnded {
        id: String,
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
    AccountSwitched {
        from: String,
        to: String,
    },
    Picked {
        model: String,
        effort: crate::Effort,
    },
    Notified(Vec<crate::inbox::Notice>),
    Notice(String),
    Moved(std::path::PathBuf),
    Tasks(Vec<TaskItem>),
    Unsteered(Vec<String>),
    ModeChanged(crate::PermissionMode),
    Verified(crate::verification::Verdict),
    Answered {
        ask: crate::AskId,
        decision: crate::Decision,
    },
    Refused {
        tool: String,
        reason: String,
    },
    Repeated {
        tool: String,
    },
    Edited(crate::changes::Edited),
    Reviewed(Vec<crate::review::Finding>),
    Branched {
        from: SessionId,
        checkpoint: u32,
        prompt: String,
    },
    Completed {
        duration_ms: u64,
        cost_usd: Option<f64>,
    },
    Failure(FailureKind),
    Failed(String),
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackgroundChange {
    Started(String),
    Ended(String),
}

impl BackgroundChange {
    pub fn event(self) -> EngineEvent {
        match self {
            Self::Started(id) => EngineEvent::BackgroundStarted { id },
            Self::Ended(id) => EngineEvent::BackgroundEnded { id },
        }
    }
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skipped: Vec<Skipped>,
}

pub const KEPT_FILE_AT_MOST: u64 = 10 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LeftAlone {
    ChangedSince,
    TooLarge,
    Unreadable,
    Failed(String),
}

impl LeftAlone {
    pub fn said(&self) -> String {
        match self {
            Self::ChangedSince => "it changed after the agent left it".to_owned(),
            Self::TooLarge => format!(
                "it was larger than {} MiB, so its earlier version was not kept",
                KEPT_FILE_AT_MOST / (1024 * 1024)
            ),
            Self::Unreadable => "it could not be read, so its earlier version was not kept".to_owned(),
            Self::Failed(error) => error.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "SkippedAsKept")]
pub struct Skipped {
    pub path: std::path::PathBuf,
    pub why: LeftAlone,
}

impl Skipped {
    pub fn new(path: impl Into<std::path::PathBuf>, why: LeftAlone) -> Self {
        Self { path: path.into(), why }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum SkippedAsKept {
    Path(std::path::PathBuf),
    Said { path: std::path::PathBuf, why: LeftAlone },
}

impl From<SkippedAsKept> for Skipped {
    fn from(kept: SkippedAsKept) -> Self {
        match kept {
            SkippedAsKept::Path(path) => Self::new(path, LeftAlone::ChangedSince),
            SkippedAsKept::Said { path, why } => Self::new(path, why),
        }
    }
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
    fn a_file_a_rewind_left_alone_says_why_and_an_older_record_without_a_reason_still_reads() {
        use super::{LeftAlone, Rewound, Skipped};
        let older: Rewound = serde_json::from_value(serde_json::json!({
            "checkpoint": 1, "prompt": "p", "restored": [], "conversation": true, "skipped": ["/repo/a.txt"]
        }))
        .expect("an older record");
        assert_eq!(older.skipped, [Skipped::new("/repo/a.txt", LeftAlone::ChangedSince)]);

        let reasons = [
            LeftAlone::ChangedSince,
            LeftAlone::TooLarge,
            LeftAlone::Unreadable,
            LeftAlone::Failed("could not restore /repo/b: denied".into()),
        ];
        let rewound = Rewound {
            checkpoint: 0,
            prompt: "p".into(),
            restored: Vec::new(),
            conversation: true,
            skipped: reasons.iter().map(|why| Skipped::new("/repo/x", why.clone())).collect(),
        };
        let again: Rewound = serde_json::from_value(serde_json::to_value(&rewound).expect("saved")).expect("read");
        assert_eq!(again, rewound);
        for why in &reasons {
            assert!(!why.said().is_empty(), "{why:?}");
        }
        assert!(LeftAlone::TooLarge.said().contains("10 MiB"));
    }

    #[test]
    fn partial_tool_input_is_recognised_inside_agents_too() {
        let piece = EngineEvent::ToolInputDelta { name: "Read".into(), piece: "{".into() };
        assert!(piece.is_partial_input());
        assert!(agent(0, piece).is_partial_input());
        assert!(!EngineEvent::TextDelta("x".into()).is_partial_input());
    }
}
