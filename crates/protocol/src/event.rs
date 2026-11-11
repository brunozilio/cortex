use std::path::PathBuf;

use domain::{CustomAgent, EngineEvent, ModelOption, RunId, Skill};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum ServerEvent {
    Run { run: RunId, event: EngineEvent },
    Models(Vec<ModelOption>),
    Settings { settings: Option<serde_json::Value> },
    Skills(Vec<Skill>),
    Agents(Vec<CustomAgent>),
    McpServers { path: PathBuf, servers: Vec<domain::mcp_config::McpServerInfo> },
    McpAuthorizeAt { name: String, url: String },
    Notices(Vec<domain::inbox::Notice>),
    Feature { feature: String, output: Result<serde_json::Value, String> },
    Permissions { path: PathBuf, permissions: domain::permission_rules::PermissionsInfo },
    Providers(crate::ProviderEvent),
    Error { message: String },
}

impl ServerEvent {
    pub fn error(message: impl Into<String>) -> Self {
        Self::Error { message: message.into() }
    }
}
