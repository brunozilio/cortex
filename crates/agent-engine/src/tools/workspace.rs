//! What a run's tools share: its folder, credentials and model, the files
//! it read, its hooks and permission rules, and what its sub-agents fork.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub struct Workspace {
    pub cwd: PathBuf,
    pub(crate) read: Arc<Mutex<HashSet<PathBuf>>>,
    pub stop: Arc<AtomicBool>,
    /// Set when the user moves the running `Bash` command to the background
    /// (Ctrl+B); shared with sub-agents. With several commands in the
    /// foreground at once (sub-agents side by side), one Ctrl+B moves the
    /// first to notice it, and each press moves one more.
    pub detach: Arc<AtomicBool>,
    pub credentials: domain::credentials::RunCredentials,
    pub model: domain::ModelOption,
    pub prompt_context: Option<crate::prompt::Environment>,
    pub hooks: crate::hooks::Hooks,
    /// `permissions` rules from settings, over the permission mode.
    pub permissions: domain::permission_rules::PermissionRules,
    /// Where `~/` in a permission rule starts.
    pub home: Option<PathBuf>,
    /// Folders read like the project's own (`/add-dir`,
    /// `additionalDirectories`).
    pub added_dirs: Vec<PathBuf>,
    /// The model to go on with when the run's is overloaded (`fallbackModel`,
    /// `--fallback-model`).
    pub fallback: Option<String>,
    /// The way to the web for `WebFetch` and `WebSearch`.
    pub web: Option<super::web::Web>,
    /// Tools left out of the turns until `ToolSearch` loads them; shared
    /// with sub-agents.
    pub deferred: Option<Arc<super::tool_search::Deferred>>,
    pub agent_name: String,
    pub session_id: String,
    pub auto_memory: bool,
    pub rules: Vec<crate::prompt::rules::Rule>,
    pub(crate) seen_rules: Arc<Mutex<HashSet<PathBuf>>>,
    pub safety: Option<Arc<crate::guard::Safety>>,
    pub advisor: Option<String>,
    /// The criteria the top-level run is judged against; never forked to
    /// sub-agents.
    pub verification: Option<crate::judge::Verification>,
    /// What the run's `Edit`s and `Write`s changed, for its checkpoint;
    /// shared with its sub-agents.
    pub files: Option<Arc<crate::checkpoints::FileHistory>>,
    /// The language servers for `LSP` and the diagnostics after an edit;
    /// shared with sub-agents.
    pub lsp: Option<crate::lsp::Lsp>,
    /// The worktree the session works in (`EnterWorktree`); the main
    /// agent's only, never forked.
    pub worktree: Option<crate::agent_runtime::WorktreeState>,
}

impl Workspace {
    pub fn new(cwd: impl Into<PathBuf>) -> Self {
        Self {
            cwd: cwd.into(),
            read: Arc::default(),
            stop: Arc::new(AtomicBool::new(false)),
            detach: Arc::default(),
            credentials: Default::default(),
            prompt_context: None,
            hooks: Default::default(),
            permissions: Default::default(),
            home: None,
            added_dirs: Vec::new(),
            fallback: None,
            web: None,
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
            stop,
            detach: Arc::clone(&self.detach),
            credentials: self.credentials.clone(),
            model: self.model.clone(),
            prompt_context: self.prompt_context.clone(),
            hooks: self.hooks.clone(),
            permissions: self.permissions.clone(),
            home: self.home.clone(),
            added_dirs: self.added_dirs.clone(),
            fallback: self.fallback.clone(),
            web: self.web.clone(),
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
        }
    }

    /// Call before a tool writes `path`, so the run's checkpoint can put it back.
    pub fn before_write(&self, path: &std::path::Path) {
        if let Some(files) = &self.files {
            files.before_write(path);
        }
    }

    /// What the file's language server finds wrong in `text` (just written
    /// to `path`), as a block to add to the tool's result; empty when
    /// there is no server or nothing wrong.
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

    pub fn has_read(&self, path: &Path) -> bool {
        self.read.lock().map(|read| read.contains(path)).unwrap_or(false)
    }
}
