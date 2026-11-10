//! Messages a backend sends to its client.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use superfik_core::Principal;
use superfik_core::{
    Connection, CustomAgent, EngineEvent, EngineInfo, Memory, MemoryId, ModelOption, PluginId, PluginSummary,
    Repository, RunId, SessionRecord, Skill, WorkspaceSnapshot,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum ServerEvent {
    Welcome { version: u32, principal: Principal },
    Run { run: RunId, event: EngineEvent },
    Models(Vec<ModelOption>),
    Engines(Vec<EngineInfo>),
    Workspace(WorkspaceSnapshot),
    Repository(Repository),
    Settings { settings: Option<serde_json::Value> },
    Skills(Vec<Skill>),
    Agents(Vec<CustomAgent>),
    Memory { path: PathBuf, memories: Vec<Memory>, learned: Vec<MemoryId> },
    MemorySearch { path: PathBuf, query: String, scores: Vec<(MemoryId, f32)> },
    Sessions { path: PathBuf, sessions: Vec<SessionRecord> },
    Plugins { plugins: Vec<PluginSummary>, connections: Vec<Connection> },
    AuthorizeAt { plugin: PluginId, url: String },
    PluginFailed { plugin: Option<PluginId>, message: String },
    Providers(crate::ProviderEvent),
    Error { message: String },
}

#[cfg(test)]
#[path = "event_tests.rs"]
mod tests;
