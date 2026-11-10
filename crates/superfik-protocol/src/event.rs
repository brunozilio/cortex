//! Messages a backend sends to its client.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use superfik_core::{CustomAgent, EngineEvent, Forgotten, Memory, MemoryId, ModelOption, RunId, Skill};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum ServerEvent {
    Run {
        run: RunId,
        event: EngineEvent,
    },
    Models(Vec<ModelOption>),
    Settings {
        settings: Option<serde_json::Value>,
    },
    Skills(Vec<Skill>),
    Agents(Vec<CustomAgent>),
    McpServers {
        path: PathBuf,
        servers: Vec<superfik_core::mcp_config::McpServerInfo>,
    },
    /// The live memory, and what was forgotten but can still be restored.
    Memory {
        path: PathBuf,
        memories: Vec<Memory>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        forgotten: Vec<Forgotten>,
    },
    MemorySearch {
        path: PathBuf,
        query: String,
        scores: Vec<(MemoryId, f32)>,
    },
    Providers(crate::ProviderEvent),
    Error {
        message: String,
    },
}

impl ServerEvent {
    pub fn error(message: impl Into<String>) -> Self {
        Self::Error { message: message.into() }
    }
}
