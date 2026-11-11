use std::path::PathBuf;

use domain::{CustomAgent, EngineEvent, Forgotten, Memory, MemoryId, ModelOption, RunId, Skill};
use serde::{Deserialize, Serialize};

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
        servers: Vec<domain::mcp_config::McpServerInfo>,
    },
    Channels {
        opened: Vec<String>,
        failed: Vec<String>,
    },
    McpAuthorizeAt {
        name: String,
        url: String,
    },
    Notices(Vec<domain::inbox::Notice>),
    Feature {
        feature: String,
        output: Result<serde_json::Value, String>,
    },
    Permissions {
        path: PathBuf,
        permissions: domain::permission_rules::PermissionsInfo,
    },
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
