pub mod ask;
pub mod builtin {
    include!(concat!(env!("OUT_DIR"), "/builtin_tools.rs"));
}
pub mod builtin_agents;
pub mod builtin_skills;
pub mod inbox;
mod lsp;
mod outside;
pub mod peers;
pub mod persisted;
pub mod plan_mode;
pub mod read_files;
mod rule_target;
pub(crate) mod script_bridge;
pub(crate) mod script_io;
pub mod script_tool;
pub mod seen_files;
pub mod send_message;
pub mod skill;
mod skill_listing;
mod skill_script;
pub mod stopped;
pub mod structured;
pub mod subagent;
pub mod subagent_config;
pub mod task;
pub mod tool_search;
pub mod watch;
mod workspace;
pub mod worktree;

use std::path::{Path, PathBuf};

use serde_json::Value;

pub use lsp::Lsp;
pub use workspace::{Network, Verification, Workspace};

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
    pub images: Vec<domain::Image>,
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

    pub fn image(image: domain::Image) -> Self {
        Self { content: format!("Image ({})", image.media_type), is_error: false, images: vec![image], shown: None }
    }

    pub fn error(content: impl Into<String>) -> Self {
        Self { content: content.into(), is_error: true, images: Vec::new(), shown: None }
    }
}

pub struct Consult<'a> {
    pub api: &'a dyn crate::model::ModelApi,
    pub request: &'a crate::model::TurnRequest,
    pub conversation: &'a [crate::transcript::Message],
    pub stop: &'a std::sync::atomic::AtomicBool,
}

impl std::fmt::Debug for Consult<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Consult").field("model", &self.request.model).finish_non_exhaustive()
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

    fn offered(&self, _workspace: &Workspace) -> bool {
        true
    }

    fn serial(&self) -> bool {
        false
    }

    fn consult(&self, _input: &Value, _workspace: &Workspace, _agent: &Consult<'_>) -> Option<Outcome> {
        None
    }

    fn script(&self) -> Option<&script_tool::ScriptTool> {
        None
    }

    fn skills(&self) -> &[skill::FoundSkill] {
        &[]
    }

    fn kept(&self) -> Option<std::sync::Arc<dyn Tool>> {
        None
    }

    fn declared_read_only(&self) -> bool {
        false
    }
}

pub fn standard() -> Vec<Box<dyn Tool>> {
    let features: Vec<Box<dyn Tool>> = vec![
        Box::new(ask::AskUserQuestion),
        Box::new(plan_mode::EnterPlanMode),
        Box::new(plan_mode::ExitPlanMode),
        Box::new(worktree::EnterWorktree),
        Box::new(worktree::ExitWorktree),
        Box::new(peers::ListSessions),
        Box::new(watch::Watch),
    ];
    builtin::tools().into_iter().chain(features).collect()
}

pub fn named(name: &str) -> Option<Box<dyn Tool>> {
    standard().into_iter().find(|tool| tool.spec().name == name)
}

pub fn describe(name: &str, data: &Value) -> String {
    let script = format!("tools/{name}.js");
    if content::builtin(&script).is_some() || content::user_text(content::user_folder().as_deref(), &script).is_some() {
        return crate::prompt::render(&script, data);
    }
    content::text(&format!("tools/{name}.md"))
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
        let names: std::collections::HashSet<String> = tools.iter().map(|tool| tool.spec().name).collect();
        assert_eq!(names.len(), tools.len());
        for tool in tools {
            assert_eq!(tool.spec().schema["type"], "object", "{}", tool.spec().name);
        }
    }
}
