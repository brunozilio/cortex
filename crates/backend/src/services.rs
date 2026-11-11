//! What a backend runs on: the engine, memory, settings, providers and the
//! host's extensions, injected so tests and the SDK can swap them.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use agent_engine::AgentEngine;

use crate::authorizer::{Authorizer, LocalAuthorizer};
use crate::extensions::Extensions;
use crate::memory::{FileMemoryStore, Memories, ModelChoice};
use crate::providers::{FileProviderStore, ModelProviders};
use crate::settings::{FileSettingsStore, SettingsStore};

#[derive(Clone)]
pub struct LocalServices {
    pub engine: Arc<dyn AgentEngine>,
    pub memory: Memories,
    /// Whether runs get the `memory_search` MCP tool.
    pub memory_tool: bool,
    pub settings: Arc<dyn SettingsStore>,
    pub extensions: Extensions,
    pub providers: ModelProviders,
    pub authorizer: Arc<dyn Authorizer>,
    /// Where downloaded MLX models are, on Apple silicon.
    pub local_models: Option<PathBuf>,
    /// The owner's home, whose `.{command}/mcp.json` holds their own MCP
    /// servers (the process's home; tests name another).
    pub home: Option<PathBuf>,
    /// The owner's sign-ins to remote MCP servers (`/mcp login`).
    pub mcp_auth: crate::McpAuth,
}

impl std::fmt::Debug for LocalServices {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalServices").finish_non_exhaustive()
    }
}

impl LocalServices {
    /// The native engine on this computer, with local models when it can
    /// run them.
    pub fn detect() -> Self {
        let local = crate::models::system();
        let data = crate::local_data_dir().unwrap_or_else(std::env::temp_dir);
        let engine = Arc::new(crate::native::engine(&data, local.runtime));
        Self { local_models: local.folder, ..Self::with_engine(engine) }
    }

    pub fn with_engine(engine: Arc<dyn AgentEngine>) -> Self {
        let data = crate::local_data_dir().unwrap_or_else(std::env::temp_dir);
        Self {
            engine,
            memory: Memories::new(
                crate::memory::system_memory_store(),
                crate::memory::local(ModelChoice::from_env(), data.join("models")),
            ),
            memory_tool: true,
            settings: Arc::new(FileSettingsStore::system()),
            extensions: Extensions::default(),
            providers: ModelProviders::system(),
            authorizer: Arc::new(LocalAuthorizer),
            local_models: None,
            home: std::env::home_dir(),
            mcp_auth: crate::McpAuth::system(),
        }
    }

    /// Keeps every store under `data` instead of the system folders (tests,
    /// or an isolated profile).
    pub fn kept_in(mut self, data: &Path) -> Self {
        self.memory = self.memory.with_store(Arc::new(FileMemoryStore::new(data.join("memory"))));
        self.settings = Arc::new(FileSettingsStore::new(data.join("users")));
        self.providers = self.providers.with_store(Arc::new(FileProviderStore::new(data.join("providers"))));
        self
    }
}

#[cfg(test)]
mod tests {
    use agent_engine::fake::FakeEngine;

    use super::*;

    #[test]
    fn kept_in_keeps_settings_in_the_given_folder() {
        let data = tempfile::tempdir().expect("temp dir");
        let services =
            LocalServices::with_engine(Arc::new(FakeEngine::new(Vec::new(), Vec::new()))).kept_in(data.path());
        let owner = domain::Principal::LocalOwner;
        services.settings.save(&owner, &serde_json::json!({"model": "m"})).expect("save");
        assert!(data.path().join("users").exists());
    }
}
