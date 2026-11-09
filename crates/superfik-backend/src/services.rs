//! The collaborators a backend needs, injected as trait objects.

use std::sync::Arc;

use superfik_engine::{EngineRegistry, PlannedEngine};
use superfik_plugins::{AgentCatalog, AgentDiscovery, SkillCatalog, SkillDiscovery};
use superfik_workspace::{
    BranchPusher, BranchSwitcher, GhCli, GitCli, GitHubClient, GitInspector, NoGitHub, Worktrees,
};

use crate::authorizer::{Authorizer, LocalAuthorizer};
use crate::channels::{Channels, FileChannelStore};
use crate::history::{FileSessionStore, Sessions};
use crate::memory::{FileMemoryStore, Memories, ModelChoice};
use crate::notes::{FileNoteStore, Notes};
use crate::plugins::Plugins;
use crate::routines::{FileRoutineStore, Routines};

fn system_channels() -> Channels {
    let mut openers: Vec<Arc<dyn superfik_channels::Opener>> = Vec::new();
    match superfik_channels::whatsapp::WhatsappOpener::new() {
        Ok(whatsapp) => openers.push(Arc::new(whatsapp)),
        Err(error) => tracing::warn!(%error, "WhatsApp unavailable"),
    }
    openers.push(Arc::new(superfik_channels::email::EmailOpener));
    let http: Arc<dyn superfik_channels::http::Http> = Arc::new(superfik_channels::http::UreqHttp::default());
    openers.push(Arc::new(superfik_channels::whatsapp_cloud::CloudOpener::new(Arc::clone(&http))));
    openers.push(Arc::new(superfik_channels::instagram::InstagramOpener::new(Arc::clone(&http))));
    openers.push(Arc::new(superfik_channels::x::XOpener::new(Arc::clone(&http))));
    openers.push(Arc::new(superfik_channels::linkedin::LinkedinOpener::new(http)));
    let root = crate::local_data_dir().unwrap_or_else(std::env::temp_dir).join("channel-sessions");
    Channels::new(openers, root, Arc::new(FileChannelStore::system()), Arc::new(crate::channels::sender::SystemClock))
}
use crate::vault::{CommandRunner, ShellRunner, VaultProviders};

#[derive(Clone)]
pub struct LocalServices {
    pub engines: EngineRegistry,
    pub git: Arc<dyn GitInspector>,
    pub switcher: Arc<dyn BranchSwitcher>,
    pub pusher: Arc<dyn BranchPusher>,
    pub github: Arc<dyn GitHubClient>,
    pub github_tokens: Arc<dyn crate::github::GitHubTokens>,
    pub skills: Arc<dyn SkillCatalog>,
    pub agents: Arc<dyn AgentCatalog>,
    pub memory: Memories,
    pub memory_tool: bool,
    pub notes: Notes,
    pub routines: Routines,
    pub worktrees: Arc<dyn Worktrees>,
    pub worktree_root: std::path::PathBuf,
    pub budget: Arc<dyn crate::Budget>,
    pub channels: Channels,
    pub background: bool,
    pub sessions: Sessions,
    pub settings: Arc<dyn crate::settings::SettingsStore>,
    pub plugins: Plugins,
    pub providers: crate::providers::ModelProviders,
    pub vaults: VaultProviders,
    pub commands: Arc<dyn CommandRunner>,
    pub authorizer: Arc<dyn Authorizer>,
    pub pages: Arc<dyn crate::PageService>,
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
            background: true,
            channels: system_channels(),
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
            switcher: git.clone(),
            pusher: git.clone(),
            worktrees: git,
            worktree_root: crate::local_data_dir().unwrap_or_else(std::env::temp_dir).join("worktrees"),
            budget: Arc::new(crate::Unbounded),
            github,
            github_tokens: Arc::new(crate::github::NoGitHubTokens),
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
            notes: Notes::new(Arc::new(FileNoteStore::system())),
            routines: Routines::new(Arc::new(FileRoutineStore::system())),
            channels: Channels::none(),
            background: false,
            sessions: Sessions::new(Arc::new(FileSessionStore::system())),
            settings: Arc::new(crate::settings::FileSettingsStore::system()),
            plugins: Plugins::system(),
            providers: crate::providers::ModelProviders::system(),
            vaults: VaultProviders::system(),
            commands: Arc::new(ShellRunner),
            authorizer: Arc::new(LocalAuthorizer),
            pages: Arc::new(crate::NoPages),
            local_models: None,
            downloads: None,
        }
    }

    pub fn acting_as(&self, owner: &superfik_core::Principal) -> Self {
        if *owner == superfik_core::Principal::LocalOwner {
            return self.clone();
        }
        let token = self.github_tokens.token(owner);
        let github = token.as_deref().and_then(|token| self.github.as_user(token));
        let pusher: Arc<dyn BranchPusher> = Arc::new(crate::github::PushingAs {
            pusher: Arc::clone(&self.pusher),
            env: token.as_deref().map(superfik_core::credentials::github_env),
        });
        Self { github: github.unwrap_or_else(|| Arc::new(NoGitHub)), pusher, ..self.clone() }
    }

    pub fn kept_in(mut self, data: &std::path::Path) -> Self {
        self.memory = self.memory.with_store(Arc::new(FileMemoryStore::new(data.join("memory"))));
        self.notes = Notes::new(Arc::new(FileNoteStore::new(data.join("notes"))));
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
        let infos: Vec<EngineKind> = services.engines.infos().iter().map(|info| info.kind).collect();
        assert_eq!(infos, vec![EngineKind::Native, EngineKind::Codex, EngineKind::Kiro, EngineKind::Copilot]);
    }

    #[test]
    fn kept_in_keeps_members_data_in_the_given_folder() {
        let data = tempfile::tempdir().expect("temp dir");
        let services = LocalServices::with_engines(EngineRegistry::new()).kept_in(data.path());
        let owner = superfik_core::Principal::LocalOwner;
        let id = superfik_core::PluginId("context7".to_owned());
        services.plugins.install(&owner, Some(&id), None);
        assert!(data.path().join("plugins").join(crate::memory::owner_key(&owner)).join("plugins.json").is_file());
    }
}
