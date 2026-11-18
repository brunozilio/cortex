mod anchors;
mod blast;
mod claims;
mod confined;
mod launcher;
mod models;
mod policy;
mod services;
#[cfg(test)]
mod tests;
mod tracking;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

pub(crate) use confined::from_domain;
pub use confined::{Confiner, confinement_of, data_folders, temp_folders};
pub use launcher::{Launch, Launcher, OnThisMachine};
pub use models::Models;
pub use policy::{Policy, env_secrets};
pub use services::{Network, Services, WebSearcher};
pub(crate) use tracking::Tracking;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verification {
    pub criteria: domain::Criteria,
    pub model: String,
    pub allow: domain::permission_rules::PermissionRules,
}

#[derive(Debug)]
pub struct Workspace {
    pub cwd: PathBuf,
    pub stop: Arc<platform::Stop>,
    pub detach: Arc<AtomicBool>,
    pub credentials: domain::credentials::RunCredentials,
    pub prompt_context: Option<crate::prompt::Environment>,
    pub hooks: crate::hooks::Hooks,
    pub home: Option<PathBuf>,
    pub added_dirs: Vec<PathBuf>,
    pub agent_name: String,
    pub observing: Option<domain::observer::Observing>,
    pub teammate: Option<crate::team::Me>,
    pub session_id: String,
    pub rules: Vec<crate::prompt::rules::Rule>,
    pub anchored_memories: Vec<domain::Memory>,
    pub blast_radius: bool,
    pub verification: Option<Verification>,
    pub worktree: Option<crate::agent_runtime::WorktreeState>,
    pub policy: Policy,
    pub models: Models,
    pub services: Services,
    pub(crate) tracking: Tracking,
}

impl Workspace {
    pub fn new(cwd: impl Into<PathBuf>) -> Self {
        Self {
            cwd: cwd.into(),
            stop: Arc::default(),
            detach: Arc::default(),
            credentials: Default::default(),
            prompt_context: None,
            hooks: Default::default(),
            home: None,
            added_dirs: Vec::new(),
            agent_name: "main".into(),
            observing: None,
            teammate: None,
            session_id: uuid::Uuid::new_v4().to_string(),
            rules: Vec::new(),
            anchored_memories: Vec::new(),
            blast_radius: false,
            verification: None,
            worktree: None,
            policy: Policy::default(),
            models: Models::default(),
            services: Services::default(),
            tracking: Tracking::default(),
        }
    }

    pub fn instructions_text(&self) -> String {
        self.prompt_context
            .iter()
            .flat_map(|context| &context.instructions)
            .map(|file| format!("Contents of {}:\n{}", file.path.display(), file.text.trim()))
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    pub fn for_request(request: &domain::PromptRequest) -> Self {
        Self {
            credentials: request.host.credentials.clone(),
            models: Models::new(request.session.model.clone()),
            ..Self::new(&request.session.cwd)
        }
    }

    pub fn fork(&self, stop: Arc<platform::Stop>) -> Self {
        Self {
            cwd: self.cwd.clone(),
            stop,
            detach: Arc::clone(&self.detach),
            credentials: self.credentials.clone(),
            prompt_context: self.prompt_context.clone(),
            hooks: self.hooks.clone(),
            home: self.home.clone(),
            added_dirs: self.added_dirs.clone(),
            agent_name: self.agent_name.clone(),
            observing: self.observing.as_ref().and_then(domain::observer::Observing::inherited),
            teammate: self.teammate.clone(),
            session_id: self.session_id.clone(),
            rules: self.rules.clone(),
            anchored_memories: self.anchored_memories.clone(),
            blast_radius: self.blast_radius,
            verification: None,
            worktree: None,
            policy: self.policy.fork(),
            models: self.models.clone(),
            services: self.services.clone(),
            tracking: self.tracking.fork(),
        }
    }

    pub fn before_write(&self, path: &Path) -> Option<String> {
        let why = self.services.files.as_ref()?.before_write(path)?;
        Some(format!(
            "\n(/rewind cannot put back the earlier version of {}: {}; it stays as it is now if this turn is rewound)",
            path.display(),
            why.said()
        ))
    }

    pub fn language_servers(&self) -> Option<&crate::lsp::Lsp> {
        self.services.lsp.as_ref().filter(|_| self.policy.trust == domain::trust::Trust::Trusted)
    }

    pub fn diagnosed(&self, path: &Path, text: &str) -> String {
        let problems = self.language_servers().and_then(|lsp| lsp.problems(path, text, &self.cwd, &self.stop));
        problems.map(|block| format!("\n\n{block}")).unwrap_or_default()
    }

    pub fn rule_instructions(&self) -> String {
        self.tracking.rule_instructions(&self.rules)
    }

    pub fn resolve(&self, path: &str) -> PathBuf {
        let path = Path::new(path);
        if path.is_absolute() { path.to_path_buf() } else { self.cwd.join(path) }
    }

    pub fn mark_read(&self, path: &Path) {
        self.tracking.mark_read(path);
    }

    pub fn mark_partly_read(&self, path: &Path) {
        self.tracking.mark_partly_read(path);
    }

    pub fn freshness(&self, path: &Path) -> crate::tools::read_files::Freshness {
        self.tracking.freshness(path)
    }

    pub fn forget_seen_files(&self) {
        self.tracking.forget_seen_files();
    }

    pub fn has_read(&self, path: &Path) -> bool {
        self.tracking.has_read(path)
    }
}
