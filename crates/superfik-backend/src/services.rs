//! The collaborators a backend needs, injected as trait objects.

use std::sync::Arc;

use superfik_engine::{EngineRegistry, PlannedEngine};
use superfik_plugins::{AgentCatalog, AgentDiscovery, SkillCatalog, SkillDiscovery};
use superfik_workspace::{BranchSwitcher, GhCli, GitCli, GitHubClient, GitInspector, NoGitHub};

use crate::authorizer::{Authorizer, LocalAuthorizer};
use crate::history::{FileSessionStore, Sessions};
use crate::memory::{FileMemoryStore, Memories, ModelChoice};
use crate::plugins::Plugins;

#[derive(Clone)]
pub struct LocalServices {
    pub engines: EngineRegistry,
    pub git: Arc<dyn GitInspector>,
    pub switcher: Arc<dyn BranchSwitcher>,
    pub github: Arc<dyn GitHubClient>,
    pub skills: Arc<dyn SkillCatalog>,
    pub agents: Arc<dyn AgentCatalog>,
    pub memory: Memories,
    pub memory_tool: bool,
    pub sessions: Sessions,
    pub settings: Arc<dyn crate::settings::SettingsStore>,
    pub plugins: Plugins,
    pub providers: crate::providers::ModelProviders,
    pub authorizer: Arc<dyn Authorizer>,
    pub local_models: Option<std::path::PathBuf>,
    pub downloads: Option<Arc<dyn crate::models::Downloads>>,
}

impl std::fmt::Debug for LocalServices {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalServices").field("engines", &self.engines).finish_non_exhaustive()
    }
}

impl LocalServices {
    pub fn detect() -> Self {
        let local = crate::models::system();
        let data = crate::local_data_dir().unwrap_or_else(std::env::temp_dir);
        let engines = EngineRegistry::new()
            .with(Arc::new(crate::native::engine(&data, local.runtime.clone())))
            .with_planned(PlannedEngine::detect_all());
        Self {
            local_models: local.folder,
            downloads: local.runtime.clone().map(|runtime| runtime as Arc<dyn crate::models::Downloads>),
            ..Self::with_engines(engines)
        }
    }

    pub fn with_engines(engines: EngineRegistry) -> Self {
        let git = Arc::new(GitCli::system());
        let github: Arc<dyn GitHubClient> = match GhCli::detect() {
            Some(gh) => Arc::new(gh),
            None => Arc::new(NoGitHub),
        };
        Self {
            engines,
            git: git.clone(),
            switcher: git,
            github,
            skills: Arc::new(SkillDiscovery::new()),
            agents: Arc::new(AgentDiscovery::new()),
            memory: Memories::new(
                Arc::new(FileMemoryStore::system()),
                crate::memory::local(
                    ModelChoice::from_env(),
                    crate::local_data_dir().unwrap_or_else(std::env::temp_dir).join("models"),
                ),
            ),
            memory_tool: true,
            sessions: Sessions::new(Arc::new(FileSessionStore::system())),
            settings: Arc::new(crate::settings::FileSettingsStore::system()),
            plugins: Plugins::system(),
            providers: crate::providers::ModelProviders::system(),
            authorizer: Arc::new(LocalAuthorizer),
            local_models: None,
            downloads: None,
        }
    }

    /// Keeps every store under `data` instead of the system folders (tests,
    /// or an isolated profile).
    pub fn kept_in(mut self, data: &std::path::Path) -> Self {
        self.memory = self.memory.with_store(Arc::new(FileMemoryStore::new(data.join("memory"))));
        self.sessions = Sessions::new(Arc::new(FileSessionStore::new(data.join("sessions"))));
        self.settings = Arc::new(crate::settings::FileSettingsStore::new(data.join("users")));
        self.plugins = self.plugins.with_store(Arc::new(crate::plugins::FilePluginStore::new(data.join("plugins"))));
        let providers = crate::providers::FileProviderStore::new(data.join("providers"));
        self.providers = self.providers.with_store(Arc::new(providers));
        self
    }
}

#[cfg(test)]
mod tests {
    use superfik_core::EngineKind;

    use super::*;

    #[test]
    fn detect_registers_only_the_native_runtime() {
        let services = LocalServices::detect();
        assert_eq!(services.engines.kinds(), vec![EngineKind::Native]);
    }

    #[test]
    fn kept_in_keeps_data_in_the_given_folder() {
        let data = tempfile::tempdir().expect("temp dir");
        let services = LocalServices::with_engines(EngineRegistry::new()).kept_in(data.path());
        let owner = superfik_core::Principal::LocalOwner;
        let id = superfik_core::PluginId("anthropic".to_owned());
        services.plugins.install(&owner, Some(&id), None);
        assert!(data.path().join("plugins").join(crate::memory::owner_key(&owner)).join("plugins.json").is_file());
    }
}
