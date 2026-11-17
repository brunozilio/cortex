use std::path::{Path, PathBuf};
use std::sync::Arc;

use agent_engine::AgentEngine;
use agent_engine::sessions::FileSessions;

use crate::data_root::DataRoot;
use crate::extensions::Extensions;
use crate::providers::ModelProviders;
use crate::settings::SettingsStore;

#[derive(Clone)]
pub struct LocalServices {
    pub engine: Arc<dyn AgentEngine>,
    pub root: DataRoot,
    pub off: crate::TurnedOff,
    pub settings: Arc<dyn SettingsStore>,
    pub catalog: Arc<dyn crate::SessionCatalog>,
    pub sessions: Arc<FileSessions>,
    pub extensions: Extensions,
    pub providers: ModelProviders,
    pub homes: Arc<dyn crate::Homes>,
    pub mcp_auth: crate::McpAuth,
}

impl std::fmt::Debug for LocalServices {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalServices").field("root", &self.root).finish_non_exhaustive()
    }
}

impl LocalServices {
    pub fn detect_in(cwd: Option<&Path>) -> Self {
        let root = DataRoot::system();
        let stores = root.stores();
        let engine = crate::native::engine(Arc::clone(&stores.sessions), root.language_servers());
        let mut services = Self::of(Arc::new(engine), root, stores);
        services.homes = Arc::new(crate::OwnerHome::system());
        services.in_project(cwd)
    }

    pub fn in_project(self, cwd: Option<&Path>) -> Self {
        let Some(cwd) = cwd.map(Path::to_path_buf).or_else(|| std::env::current_dir().ok()) else { return self };
        let _ = crate::native::declare_providers(self.home().as_deref(), &cwd);
        self.turned_off_in(&cwd)
    }

    pub fn turned_off_in(self, cwd: &Path) -> Self {
        if let Ok(settings) = agent_engine::prompt::settings::load(cwd, self.home().as_deref(), &[]) {
            self.off.follow(&settings.features);
        }
        self
    }

    pub fn with_engine(engine: Arc<dyn AgentEngine>) -> Self {
        Self::in_root(engine, DataRoot::in_memory())
    }

    pub fn in_root(engine: Arc<dyn AgentEngine>, root: DataRoot) -> Self {
        let stores = root.stores();
        Self::of(engine, root, stores)
    }

    fn of(engine: Arc<dyn AgentEngine>, root: DataRoot, stores: crate::data_root::Stores) -> Self {
        let http = Arc::new(agent_engine::model::http::NetHttp);
        Self {
            engine,
            root,
            off: crate::TurnedOff::default(),
            settings: stores.settings,
            catalog: stores.catalog,
            sessions: stores.sessions,
            extensions: Extensions::default(),
            providers: ModelProviders::new(stores.providers, Arc::clone(&stores.secrets)),
            homes: Arc::new(crate::OwnerHome::none()),
            mcp_auth: crate::McpAuth::new(stores.secrets, http),
        }
    }

    pub fn home(&self) -> Option<PathBuf> {
        self.homes.home()
    }

    pub fn without(mut self, feature: &str) -> Self {
        self.off.insert(feature.to_owned());
        self
    }

    pub fn kept_in(self, data: &Path) -> Self {
        let root = DataRoot::under(data);
        let stores = root.stores();
        Self {
            root,
            settings: stores.settings,
            catalog: stores.catalog,
            sessions: stores.sessions,
            providers: self.providers.with_store(stores.providers).with_secrets(Arc::clone(&stores.secrets)),
            mcp_auth: self.mcp_auth.with_secrets(stores.secrets),
            ..self
        }
    }
}

#[cfg(test)]
mod tests {
    use agent_engine::fake::FakeEngine;

    use super::*;

    fn engine() -> Arc<dyn AgentEngine> {
        Arc::new(FakeEngine::new(Vec::new(), Vec::new()))
    }

    #[test]
    fn kept_in_keeps_everything_in_the_given_folder() {
        let data = tempfile::tempdir().expect("temp dir");
        let services = LocalServices::with_engine(engine()).kept_in(data.path());
        llm_providers::testing::install();

        services.settings.save(&serde_json::json!({"model": "m"})).expect("save");
        services.catalog.update(&mut |catalog| {
            catalog.started(&domain::SessionId("s".into()), Path::new("/work"), 1);
        });
        services.providers.connect(domain::ModelProvider::new("keyed"), "sk-proj-0123456789abcd");

        for kept in ["users/local/settings.json", "users/local/sessions.json", "providers/local/providers.json"] {
            assert!(data.path().join(kept).is_file(), "{kept}");
        }
        assert!(data.path().join("secrets.json").is_file() && data.path().join("secrets.key").is_file());
        assert!(services.root.folders().iter().all(|folder| folder.starts_with(data.path())));
    }

    #[test]
    fn services_built_for_a_test_keep_nothing_on_this_machine() {
        let system = DataRoot::system();
        let before: Vec<bool> = system.folders().iter().map(|folder| folder.exists()).collect();

        let services = LocalServices::with_engine(engine());
        llm_providers::testing::install();
        services.settings.save(&serde_json::json!({"model": "m"})).expect("save");
        services.providers.connect(domain::ModelProvider::new("keyed"), "sk-proj-0123456789abcd");
        services.catalog.update(&mut |catalog| {
            catalog.started(&domain::SessionId("s".into()), Path::new("/work"), 1);
        });

        assert!(services.root.keeps_nothing());
        assert_eq!(services.home(), None, "the machine's home is not read either");
        assert!(services.root.folders().iter().all(|folder| !system.holds(folder) && !folder.exists()));
        assert_eq!(services.settings.load(), Some(serde_json::json!({"model": "m"})), "kept, in memory");
        let after: Vec<bool> = system.folders().iter().map(|folder| folder.exists()).collect();
        assert_eq!(before, after, "nothing was made where the user's data lives");
    }

    #[test]
    fn a_feature_the_owner_turned_off_is_off_for_the_backend_too() {
        let home = tempfile::tempdir().expect("home");
        let repo = tempfile::tempdir().expect("repo");
        std::fs::create_dir_all(home.path().join(brand::DIR)).expect("dir");
        let settings =
            serde_json::json!({ "features": { "workspace": false, "memory": true, "suggested-rules": false } });
        std::fs::write(home.path().join(brand::DIR).join("settings.json"), settings.to_string()).expect("write");
        let mut services = LocalServices::with_engine(engine());
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
        let mut services = LocalServices::with_engine(engine());
        services.homes = Arc::new(crate::OwnerHome::at(home.path()));
        assert_ne!(std::env::current_dir().ok().as_deref(), Some(repo.path()));

        let elsewhere = services.clone().in_project(None);
        assert!(!elsewhere.off.contains("workspace"), "the process's own folder says nothing about it");

        let asked = services.in_project(Some(repo.path()));
        assert!(asked.off.contains("workspace"));
        assert!(elsewhere.off.contains("workspace"), "services cloned from one another follow the same settings");
    }
}
