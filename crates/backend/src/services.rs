use std::path::{Path, PathBuf};
use std::sync::Arc;

use agent_engine::AgentEngine;

use crate::authorizer::{Authorizer, LocalAuthorizer};
use crate::extensions::Extensions;
use crate::providers::{FileProviderStore, ModelProviders};
use crate::settings::{FileSettingsStore, SettingsStore};

#[derive(Clone)]
pub struct LocalServices {
    pub engine: Arc<dyn AgentEngine>,
    pub data: Option<PathBuf>,
    pub off: std::collections::BTreeSet<String>,
    pub settings: Arc<dyn SettingsStore>,
    pub catalog: Arc<dyn crate::SessionCatalog>,
    pub extensions: Extensions,
    pub providers: ModelProviders,
    pub authorizer: Arc<dyn Authorizer>,
    pub homes: Arc<dyn crate::Homes>,
    pub mcp_auth: crate::McpAuth,
}

impl std::fmt::Debug for LocalServices {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalServices").finish_non_exhaustive()
    }
}

impl LocalServices {
    pub fn detect() -> Self {
        Self::detect_in(None)
    }

    pub fn detect_in(cwd: Option<&Path>) -> Self {
        let data = crate::local_data_dir().unwrap_or_else(std::env::temp_dir);
        let mut services = Self::with_engine(Arc::new(crate::native::engine(&data)));
        services.catalog = Arc::new(crate::catalog::FileSessionCatalog::system());
        services.in_project(cwd)
    }

    pub fn in_project(self, cwd: Option<&Path>) -> Self {
        let Some(cwd) = cwd.map(Path::to_path_buf).or_else(|| std::env::current_dir().ok()) else { return self };
        let _ = crate::native::declare_providers(self.home(&domain::Principal::LocalOwner).as_deref(), &cwd);
        self.turned_off_in(&cwd)
    }

    pub fn turned_off_in(mut self, cwd: &Path) -> Self {
        if let Ok(settings) =
            agent_engine::prompt::settings::load(cwd, self.home(&domain::Principal::LocalOwner).as_deref(), &[])
        {
            self.off.extend(settings.features.off().flat_map(|name| [name.to_owned(), name.replace('-', "_")]));
        }
        self
    }

    pub fn with_engine(engine: Arc<dyn AgentEngine>) -> Self {
        Self {
            engine,
            data: crate::dirs::app_data_dir(),
            off: std::collections::BTreeSet::new(),
            settings: Arc::new(FileSettingsStore::system()),
            catalog: Arc::new(crate::catalog::InMemorySessionCatalog::default()),
            extensions: Extensions::default(),
            providers: ModelProviders::system(),
            authorizer: Arc::new(LocalAuthorizer),
            homes: Arc::new(crate::OwnerHome::system()),
            mcp_auth: crate::McpAuth::system(),
        }
    }

    pub fn home(&self, owner: &domain::Principal) -> Option<PathBuf> {
        self.homes.home(owner)
    }

    pub fn without(mut self, feature: &str) -> Self {
        self.off.insert(feature.to_owned());
        self
    }

    pub fn kept_in(mut self, data: &Path) -> Self {
        self.data = Some(data.to_path_buf());
        self.settings = Arc::new(FileSettingsStore::new(data.join("users")));
        self.catalog = Arc::new(crate::catalog::FileSessionCatalog::new(data.join("users")));
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

    #[test]
    fn a_feature_the_owner_turned_off_is_off_for_the_backend_too() {
        let home = tempfile::tempdir().expect("home");
        let repo = tempfile::tempdir().expect("repo");
        std::fs::create_dir_all(home.path().join(brand::DIR)).expect("dir");
        let settings =
            serde_json::json!({ "features": { "workspace": false, "memory": true, "suggested-rules": false } });
        std::fs::write(home.path().join(brand::DIR).join("settings.json"), settings.to_string()).expect("write");
        let mut services = LocalServices::with_engine(Arc::new(FakeEngine::new(Vec::new(), Vec::new())));
        services.homes = Arc::new(crate::OwnerHome::at(home.path()));

        let services = services.turned_off_in(repo.path());

        assert!(services.off.contains("workspace"));
        assert!(!services.off.contains("memory"));
        assert!(services.off.contains("suggested_rules"), "a flag names its feature folder with hyphens");
    }

    #[test]
    fn the_folder_asked_for_decides_what_is_off_not_the_one_the_process_started_in() {
        let home = tempfile::tempdir().expect("home");
        let repo = tempfile::tempdir().expect("repo");
        std::fs::create_dir_all(repo.path().join(brand::DIR)).expect("dir");
        let settings = serde_json::json!({ "features": { "workspace": false } });
        std::fs::write(repo.path().join(brand::DIR).join("settings.local.json"), settings.to_string()).expect("write");
        let mut services = LocalServices::with_engine(Arc::new(FakeEngine::new(Vec::new(), Vec::new())));
        services.homes = Arc::new(crate::OwnerHome::at(home.path()));
        assert_ne!(std::env::current_dir().ok().as_deref(), Some(repo.path()));

        let elsewhere = services.clone().in_project(None);
        let asked = services.in_project(Some(repo.path()));

        assert!(!elsewhere.off.contains("workspace"), "the process's own folder says nothing about it");
        assert!(asked.off.contains("workspace"));
    }
}
