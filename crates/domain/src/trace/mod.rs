use serde::{Deserialize, Serialize};

use crate::EngineEvent;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Record {
    Prompted { at_ms: u64, prompt: String },
    Shown { text: String, attached: Vec<String> },
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
