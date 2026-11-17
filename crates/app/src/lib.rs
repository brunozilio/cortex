pub mod api;
mod catalog;
mod client;
mod data_root;
mod db;
mod degraded;
mod dirs;
mod extensions;
mod features;
mod git;
mod handler;
mod homes;
#[cfg(test)]
mod legacy_layout_tests;
mod mcp_admin;
mod mcp_auth;
mod native;
mod permissions;
mod providers;
mod secrets;
mod services;
mod settings;
mod turned_off;

pub use api::features::flags::ListedFlag;
pub use api::features::plugins::{Plugin, PluginsReply, PluginsRequest};
pub use api::features::shell::BackgroundCommand;
pub use api::{Backend, BackendError, Wake};
pub use catalog::{InMemorySessionCatalog, SessionCatalog, SqliteSessionCatalog};
pub use client::{Builder, Error, Headless, ProjectMemory, Run, RunControl, Sdk, Session, SignedIn, Spending};
pub use data_root::DataRoot;
pub use db::Db;
pub use degraded::Degraded;
pub use dirs::project_key;
pub use domain::mcp_config::{McpOrigin, McpServerInfo, McpState};
pub use domain::memory::seed::Candidate as SeedCandidate;
pub use domain::memory::{Forgotten, Memory, MemoryEdit, MemoryId, MemoryKind, Revision};
pub use domain::provider::{ModelProvider, PortHolder, ProviderSummary};
pub use domain::schedule::{Scheduled, When};
pub use domain::{
    AgentAsk, AskId, AskKind, Choice, ChoreAnswer, Criteria, CriteriaSource, CustomAgent, Decision, Effort,
    EngineEvent, HeadsUp, Image, MAX_IMAGE_BYTES, ModelOption as Model, PermissionMode, Question, Rewound, SessionId,
    ToolCall, Verdict,
};
pub use extensions::Extensions;
pub use handler::CommandHandler;
pub use handler::local::LocalBackend;
pub use handler::outbox::{Outbox, Replies};
pub use homes::{Homes, OwnerHome};
pub use mcp_auth::McpAuth;
pub use providers::{InMemoryProviderStore, ModelCatalog, ModelProviders, ProviderStore};
pub use secrets::{InMemorySecretStore, SecretStore, Secrets};
pub use services::LocalServices;
pub use settings::{SettingsChange, SettingsStore};
pub use turned_off::TurnedOff;
