mod lane;
mod route;

use serde::{Deserialize, Serialize};

pub use self::route::build;
use crate::billing::ModelUsage;
use crate::usage::Compaction;
use crate::{AgentAsk, AgentStatus, EngineEvent, ToolCall};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Record {
    Prompted { at_ms: u64, prompt: String },
    Event { at_ms: u64, event: EngineEvent },
    Restarted,
    Truncated { keep: usize },
    Called { at_ms: u64, agent: Option<usize>, call: ModelCall },
    Opened { at_ms: u64, run: RunInfo },
    Partial,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RunInfo {
    pub model: String,
    pub provider: String,
    pub permission: String,
    pub effort: String,
    pub cwd: String,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub recent: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ModelCall {
    pub model: String,
    pub stop: String,
    pub messages: usize,
    pub max_tokens: u32,
    pub effort: String,
    pub system_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    pub tools_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Trace {
    pub lane: Lane,
    pub totals: Totals,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Totals {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub cost_usd: f64,
    pub started_ms: u64,
    pub ended_ms: u64,
}

impl Totals {
    fn add(&mut self, usage: &ModelUsage) {
        self.input += usage.input;
        self.output += usage.output;
        self.cache_read += usage.cache_read;
        self.cache_write += usage.cache_write;
        self.cost_usd += usage.estimated_cost_usd.unwrap_or_default();
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Lane {
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Step {
    pub nodes: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Node {
    pub at_ms: u64,
    pub until_ms: Option<u64>,
    pub failed: bool,
    #[serde(flatten)]
    pub kind: Kind,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Agent {
    pub name: String,
    pub summary: String,
    pub prompt: String,
    pub status: Option<AgentStatus>,
    pub lane: Lane,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Kind {
    Prompt {
        text: String,
        run: Option<RunInfo>,
    },
    Turn {
        text: String,
        thinking: String,
        usage: Option<ModelUsage>,
        call: Option<ModelCall>,
    },
    Tool {
        call: ToolCall,
        output: Option<String>,
        logs: Vec<String>,
        usage: Vec<ModelUsage>,
        calls: Vec<ModelCall>,
        asks: Vec<AgentAsk>,
        agent: Option<Box<Agent>>,
    },
    Agent(Box<Agent>),
    Compacted(Compaction),
    Rewound {
        keep: Option<usize>,
        checkpoint: Option<u32>,
        prompt: Option<String>,
        restored: Vec<String>,
    },
    Steered {
        text: String,
    },
    FellBack {
        model: String,
        from: Option<String>,
    },
    Failed {
        error: String,
    },
    Cancelled,
    Completed {
        duration_ms: u64,
        cost_usd: Option<f64>,
    },
    Note {
        title: String,
        text: String,
    },
}

impl Node {
    pub fn label(&self) -> String {
        match &self.kind {
            Kind::Prompt { .. } => "prompt".into(),
            Kind::Turn { .. } => "turn".into(),
            Kind::Tool { call, .. } => format!("tool {}", call.name),
            Kind::Agent(_) => "agent".into(),
            Kind::Compacted(_) => "compacted".into(),
            Kind::Rewound { .. } => "rewound".into(),
            Kind::Steered { .. } => "steered".into(),
            Kind::FellBack { .. } => "fell_back".into(),
            Kind::Failed { .. } => "failed".into(),
            Kind::Cancelled => "cancelled".into(),
            Kind::Completed { .. } => "completed".into(),
            Kind::Note { title, .. } => title.clone(),
        }
    }
}

#[cfg(test)]
mod tests;
