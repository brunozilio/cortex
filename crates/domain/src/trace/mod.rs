mod build;

use serde::{Deserialize, Serialize};

pub use self::build::build;
use crate::billing::ModelUsage;
use crate::usage::Compaction;
use crate::{AgentStatus, EngineEvent, ToolCall};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Record {
    Prompted { at_ms: u64, prompt: String },
    Event { at_ms: u64, event: EngineEvent },
    Restarted,
    Truncated { keep: usize },
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
    },
    Turn {
        text: String,
        thinking: String,
        usage: Option<ModelUsage>,
    },
    Tool {
        call: ToolCall,
        output: Option<String>,
        logs: Vec<String>,
        usage: Vec<ModelUsage>,
        agent: Option<Box<Agent>>,
    },
    Agent(Box<Agent>),
    Compacted(Compaction),
    Rewound {
        keep: usize,
    },
    Steered {
        text: String,
    },
    FellBack {
        model: String,
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
