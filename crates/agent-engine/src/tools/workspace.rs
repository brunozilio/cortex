use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verification {
    pub criteria: domain::Criteria,
    pub model: String,
}

#[derive(Debug)]
pub struct Workspace {
    pub cwd: PathBuf,
    pub(crate) read: Arc<Mutex<HashSet<PathBuf>>>,
    pub(crate) seen_files: Arc<super::seen_files::SeenFiles>,
    pub stop: Arc<AtomicBool>,
    pub detach: Arc<AtomicBool>,
    pub credentials: domain::credentials::RunCredentials,
    pub model: domain::ModelOption,
    pub prompt_context: Option<crate::prompt::Environment>,
    pub hooks: crate::hooks::Hooks,
    pub permissions: domain::permission_rules::PermissionRules,
    pub home: Option<PathBuf>,
    pub added_dirs: Vec<PathBuf>,
    pub fallbacks: Vec<String>,
    pub small_model: Option<String>,
    pub network: Option<Network>,
    pub deferred: Option<Arc<super::tool_search::Deferred>>,
    pub agent_name: String,
    pub session_id: String,
    pub auto_memory: bool,
    pub rules: Vec<crate::prompt::rules::Rule>,
    pub(crate) seen_rules: Arc<Mutex<HashSet<PathBuf>>>,
    pub safety: Option<Arc<crate::guard::Safety>>,
    pub advisor: Option<String>,
    pub verification: Option<Verification>,
    pub files: Option<Arc<crate::checkpoints::FileHistory>>,
    pub lsp: Option<crate::lsp::Lsp>,
    pub worktree: Option<crate::agent_runtime::WorktreeState>,
    pub scripts: super::script_tool::ScriptAccess,
}

impl Workspace {
    pub fn new(cwd: impl Into<PathBuf>) -> Self {
        Self {
            cwd: cwd.into(),
            read: Arc::default(),
            seen_files: Arc::default(),
            stop: Arc::new(AtomicBool::new(false)),
            detach: Arc::default(),
            credentials: Default::default(),
            prompt_context: None,
            hooks: Default::default(),
            permissions: Default::default(),
            home: None,
            added_dirs: Vec::new(),
            fallbacks: Vec::new(),
            small_model: None,
            network: None,
            deferred: None,
            agent_name: "main".into(),
            session_id: uuid::Uuid::new_v4().to_string(),
            auto_memory: true,
            rules: Vec::new(),
            seen_rules: Arc::default(),
            model: domain::ModelOption::new("unknown", "Unknown"),
            safety: None,
            advisor: None,
            verification: None,
            files: None,
            lsp: None,
            worktree: None,
            scripts: super::script_tool::ScriptAccess::default(),
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
        Self { credentials: request.credentials.clone(), model: request.model.clone(), ..Self::new(&request.cwd) }
    }

    pub fn fork(&self, stop: Arc<AtomicBool>) -> Self {
        Self {
            cwd: self.cwd.clone(),
            read: self.read.clone(),
            seen_files: Arc::default(),
            stop,
            detach: Arc::clone(&self.detach),
            credentials: self.credentials.clone(),
            model: self.model.clone(),
            prompt_context: self.prompt_context.clone(),
            hooks: self.hooks.clone(),
            permissions: self.permissions.clone(),
            home: self.home.clone(),
            added_dirs: self.added_dirs.clone(),
            fallbacks: self.fallbacks.clone(),
            small_model: self.small_model.clone(),
            network: self.network.clone(),
            deferred: self.deferred.clone(),
            agent_name: self.agent_name.clone(),
            session_id: self.session_id.clone(),
            auto_memory: self.auto_memory,
            rules: self.rules.clone(),
            seen_rules: Arc::default(),
            safety: self.safety.as_ref().map(|safety| Arc::new(safety.fork())),
            advisor: self.advisor.clone(),
            verification: None,
            files: self.files.clone(),
            lsp: self.lsp.clone(),
            worktree: None,
            scripts: self.scripts,
        }
    }

    pub fn before_write(&self, path: &std::path::Path) {
        if let Some(files) = &self.files {
            files.before_write(path);
        }
    }

    pub fn diagnosed(&self, path: &Path, text: &str) -> String {
        let problems = self.lsp.as_ref().and_then(|lsp| lsp.problems(path, text, &self.cwd, &self.stop));
        problems.map(|block| format!("\n\n{block}")).unwrap_or_default()
    }

    pub fn rule_instructions(&self) -> String {
        let seen = self.seen_rules.lock().expect("project rules");
        self.rules
            .iter()
            .filter(|r| r.patterns.is_empty() || seen.contains(&r.path))
            .map(|r| format!("\n\n# Project rule: {}\n{}", r.path.display(), r.text))
            .collect::<Vec<_>>()
            .join("")
    }

    pub fn resolve(&self, path: &str) -> PathBuf {
        let path = Path::new(path);
        if path.is_absolute() { path.to_path_buf() } else { self.cwd.join(path) }
    }

    pub fn mark_read(&self, path: &Path) {
        if let Ok(mut read) = self.read.lock() {
            read.insert(path.to_path_buf());
        }
    }

    pub fn forget_seen_files(&self) {
        self.seen_files.forget();
    }

    pub fn has_read(&self, path: &Path) -> bool {
        self.read.lock().map(|read| read.contains(path)).unwrap_or(false)
    }
}

#[derive(Clone)]
pub struct Network(pub std::sync::Arc<dyn crate::model::http::Http>);

impl std::fmt::Debug for Network {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Network")
    }
}
