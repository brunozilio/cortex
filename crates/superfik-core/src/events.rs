//! Events emitted by an engine while a run is in progress.

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
    /// The model's reasoning as it streams (when the provider shows it).
    ThinkingDelta(String),
    ToolStarted(ToolCall),
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
    Asked(AgentAsk),
    AskWithdrawn(AskId),
    Completed {
        duration_ms: u64,
        cost_usd: Option<f64>,
    },
    Failure(FailureKind),
    Failed(String),
    Cancelled,
}

impl EngineEvent {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed { .. } | Self::Failed(_) | Self::Cancelled)
    }
}
