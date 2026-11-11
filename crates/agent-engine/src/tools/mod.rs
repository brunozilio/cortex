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
pub mod inbox;
mod lsp;
mod monitor;
pub mod notebook;
mod outside;
mod pdf;
pub mod peers;
pub mod plan_mode;
pub mod push_notification;
mod read;
mod rule_target;
pub mod schedules;
pub mod send_message;
pub mod shells;
mod shown_diff;
pub mod skill;
pub mod structured;
pub mod subagent;
pub mod subagent_config;
pub mod task;
pub mod tool_search;
pub mod web;
mod workspace;
pub mod worktree;
mod write;

use std::path::{Path, PathBuf};

use serde_json::Value;

pub use bash::Bash;
pub use edit::Edit;
pub use glob::Glob;
pub use grep::Grep;
pub use lsp::Lsp;
pub use read::Read;
pub use shells::{BashOutput, KillShell};
pub use workspace::Workspace;
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
    pub images: Vec<domain::Image>,
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

    pub fn image(image: domain::Image) -> Self {
        Self { content: format!("Image ({})", image.media_type), is_error: false, images: vec![image], shown: None }
    }

    pub fn error(content: impl Into<String>) -> Self {
        Self { content: content.into(), is_error: true, images: Vec::new(), shown: None }
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
        Box::new(plan_mode::EnterPlanMode),
        Box::new(plan_mode::ExitPlanMode),
        Box::new(worktree::EnterWorktree),
        Box::new(worktree::ExitWorktree),
        Box::new(schedules::CronCreate),
        Box::new(schedules::CronList),
        Box::new(schedules::CronDelete),
        Box::new(schedules::ScheduleWakeup),
        Box::new(push_notification::PushNotification),
        Box::new(peers::ListSessions),
        Box::new(notebook::NotebookEdit),
        Box::new(monitor::Monitor),
        Box::new(web::WebFetch),
        Box::new(web::WebSearch),
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
        let names: std::collections::HashSet<String> = tools.iter().map(|tool| tool.spec().name).collect();
        assert_eq!(names.len(), tools.len());
        for tool in tools {
            assert_eq!(tool.spec().schema["type"], "object", "{}", tool.spec().name);
        }
    }
}
