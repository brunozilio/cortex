//! Messages a backend sends to its client.

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
    /// The client's open channels, and why those asked for are not.
    Channels {
        opened: Vec<String>,
        failed: Vec<String>,
    },
    /// Where to sign in to the MCP server `name` (open it in a browser).
    McpAuthorizeAt {
        name: String,
        url: String,
    },
    /// Background commands: id, command, still running.
    Shells(Vec<(String, String, bool)>),
    /// Scheduled prompts: those now due (to send), and all still waiting.
    Schedules {
        due: Vec<domain::schedule::Scheduled>,
        scheduled: Vec<domain::schedule::Scheduled>,
        /// With `take_due`, what reached the client's inbox (monitors,
        /// background commands, channels, other sessions), to send as a turn.
        #[serde(default)]
        notices: Vec<domain::inbox::Notice>,
    },
    Permissions {
        path: PathBuf,
        permissions: domain::permission_rules::PermissionsInfo,
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
