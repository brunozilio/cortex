//! The agent's tools. Names, parameters and behaviour follow the tools
//! coding agents already know (`Read`, `Edit`, `Bash`…), so skills, agents
//! and the chat's step labels work unchanged; descriptions are our own.

pub mod advisor;
pub mod ask;
mod bash;
pub mod builtin_agents;
pub mod builtin_skills;
mod edit;
mod glob;
mod grep;
mod outside;
mod read;
pub mod shells;
mod shown_diff;
pub mod skill;
pub mod structured;
pub mod send_message;
pub mod subagent;
pub mod subagent_config;
pub mod task;
mod write;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use serde_json::Value;

pub use bash::Bash;
pub use edit::Edit;
pub use glob::Glob;
pub use grep::Grep;
pub use read::Read;
pub use shells::{BashOutput, KillShell};
pub use write::Write;

use crate::model::ToolSpec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    Reads,
    Edits,
    Executes,
    ReadsOutside,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Outcome {
    pub content: String,
    pub is_error: bool,
    pub images: Vec<superfik_core::Image>,
    /// What the user sees instead of `content` (an edit's diff); the model
    /// never reads it.
    #[serde(default)]
    pub shown: Option<String>,
}

impl Outcome {
    pub fn ok(content: impl Into<String>) -> Self {
        Self { content: content.into(), is_error: false, images: Vec::new(), shown: None }
    }

    pub fn showing(mut self, shown: impl Into<String>) -> Self {
        self.shown = Some(shown.into());
        self
    }

    pub fn image(image: superfik_core::Image) -> Self {
        Self { content: format!("Image ({})", image.media_type), is_error: false, images: vec![image], shown: None }
    }

    pub fn error(content: impl Into<String>) -> Self {
        Self { content: content.into(), is_error: true, images: Vec::new(), shown: None }
    }
}

#[derive(Debug)]
pub struct Workspace {
    pub cwd: PathBuf,
    pub(crate) read: Arc<Mutex<HashSet<PathBuf>>>,
    pub stop: Arc<AtomicBool>,
    pub credentials: superfik_core::credentials::RunCredentials,
    pub model: superfik_core::ModelOption,
    pub prompt_context: Option<crate::prompt::Environment>,
    pub hooks: crate::hooks::Hooks,
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
}

impl Workspace {
    pub fn new(cwd: impl Into<PathBuf>) -> Self {
        Self {
            cwd: cwd.into(),
            read: Arc::default(),
            stop: Arc::new(AtomicBool::new(false)),
            credentials: Default::default(),
            prompt_context: None,
            hooks: Default::default(),
            agent_name: "main".into(),
            session_id: uuid::Uuid::new_v4().to_string(),
            auto_memory: true,
            rules: Vec::new(),
            seen_rules: Arc::default(),
            model: superfik_core::ModelOption::new("unknown", "Unknown"),
            safety: None,
            advisor: None,
            verification: None,
            files: None,
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

    pub fn for_request(request: &superfik_core::PromptRequest) -> Self {
        Self { credentials: request.credentials.clone(), model: request.model.clone(), ..Self::new(&request.cwd) }
    }

    pub fn fork(&self, stop: Arc<AtomicBool>) -> Self {
        Self {
            cwd: self.cwd.clone(),
            read: self.read.clone(),
            stop,
            credentials: self.credentials.clone(),
            model: self.model.clone(),
            prompt_context: self.prompt_context.clone(),
            hooks: self.hooks.clone(),
            agent_name: self.agent_name.clone(),
            session_id: self.session_id.clone(),
            auto_memory: self.auto_memory,
            rules: self.rules.clone(),
            seen_rules: Arc::default(),
            safety: self.safety.as_ref().map(|safety| Arc::new(safety.fork())),
            advisor: self.advisor.clone(),
            verification: None,
            files: self.files.clone(),
        }
    }

    /// Call before a tool writes `path`, so the run's checkpoint can put it back.
    pub fn before_write(&self, path: &std::path::Path) {
        if let Some(files) = &self.files {
            files.before_write(path);
        }
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

pub trait Tool: Send + Sync {
    fn spec(&self) -> ToolSpec;

    fn access(&self) -> Access;

    fn reaches_outside(&self, _input: &Value, _workspace: &Workspace) -> Option<PathBuf> {
        None
    }

    fn summary(&self, input: &Value) -> String;

    fn run(&self, input: &Value, workspace: &Workspace) -> Outcome;
}

pub fn standard() -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(Read),
        Box::new(Write),
        Box::new(Edit),
        Box::new(Glob),
        Box::new(Grep),
        Box::new(Bash),
        Box::new(BashOutput),
        Box::new(KillShell),
        Box::new(ask::AskUserQuestion),
    ]
}

pub(crate) fn field<'a>(input: &'a Value, name: &str) -> Option<&'a str> {
    input.get(name).and_then(Value::as_str)
}

pub(crate) fn required<'a>(input: &'a Value, name: &str) -> Result<&'a str, Outcome> {
    field(input, name).ok_or_else(|| Outcome::error(format!("Missing required parameter `{name}`.")))
}

pub(crate) fn clip(text: &str, limit: usize) -> String {
    match text.char_indices().nth(limit) {
        None => text.to_owned(),
        Some((at, _)) => format!("{}\n… [{} more characters cut]", &text[..at], text[at..].chars().count()),
    }
}

pub(crate) fn shown(path: &Path, workspace: &Workspace) -> String {
    path.strip_prefix(&workspace.cwd).unwrap_or(path).display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_paths_and_remembers_reads() {
        let workspace = Workspace::new("/repo");
        assert_eq!(workspace.resolve("src/a.rs"), PathBuf::from("/repo/src/a.rs"));
        assert_eq!(workspace.resolve("/etc/hosts"), PathBuf::from("/etc/hosts"));
        assert!(!workspace.has_read(Path::new("/repo/a")));
        workspace.mark_read(Path::new("/repo/a"));
        assert!(workspace.has_read(Path::new("/repo/a")));
    }

    #[test]
    fn clips_long_text() {
        assert_eq!(clip("abc", 5), "abc");
        assert_eq!(clip("abcdef", 3), "abc\n… [3 more characters cut]");
    }

    #[test]
    fn every_tool_has_a_distinct_name_and_an_object_schema() {
        let tools = standard();
        let names: HashSet<String> = tools.iter().map(|tool| tool.spec().name).collect();
        assert_eq!(names.len(), tools.len());
        for tool in tools {
            assert_eq!(tool.spec().schema["type"], "object", "{}", tool.spec().name);
        }
    }
}
