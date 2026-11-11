use std::path::{Path, PathBuf};
use std::sync::Arc;

use agent_engine::AgentEngine;

use crate::authorizer::{Authorizer, LocalAuthorizer};
use crate::extensions::Extensions;
use crate::memory::{FileMemoryStore, Memories};
use crate::providers::{FileProviderStore, ModelProviders};
use crate::settings::{FileSettingsStore, SettingsStore};

#[derive(Clone)]
pub struct LocalServices {
    pub engine: Arc<dyn AgentEngine>,
    pub memory: Memories,
    pub memory_tool: bool,
    pub settings: Arc<dyn SettingsStore>,
    pub extensions: Extensions,
    pub providers: ModelProviders,
    pub authorizer: Arc<dyn Authorizer>,
    pub home: Option<PathBuf>,
    pub mcp_auth: crate::McpAuth,
}

impl std::fmt::Debug for LocalServices {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalServices").finish_non_exhaustive()
    }
}

impl LocalServices {
    pub fn detect() -> Self {
        let data = crate::local_data_dir().unwrap_or_else(std::env::temp_dir);
        if let Ok(cwd) = std::env::current_dir() {
            let _ = crate::native::declare_providers(std::env::home_dir().as_deref(), &cwd);
        }
        Self::with_engine(Arc::new(crate::native::engine(&data)))
    }

    pub fn with_engine(engine: Arc<dyn AgentEngine>) -> Self {
        Self {
            engine,
            memory: Memories::plain(crate::memory::system_memory_store()),
            memory_tool: true,
            settings: Arc::new(FileSettingsStore::system()),
            extensions: Extensions::default(),
            providers: ModelProviders::system(),
            authorizer: Arc::new(LocalAuthorizer),
            home: std::env::home_dir(),
            mcp_auth: crate::McpAuth::system(),
        }
    }

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
