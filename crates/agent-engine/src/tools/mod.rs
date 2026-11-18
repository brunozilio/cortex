pub mod after;
pub mod ask;
pub mod builtin;
pub mod builtin_agents;
#[cfg(test)]
mod builtin_skills;
pub mod checked;
pub mod host_judge;
pub mod inbox;
mod lsp;
pub mod names;
pub mod observer_report;
mod outside;
pub mod peers;
pub mod persisted;
pub mod plan_mode;
pub mod progress;
pub mod raw_text;
pub mod read_files;
pub mod repl;
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

use std::path::Path;

use serde_json::Value;

pub use domain::permission::{Declared, Effect, SpawnKind};
pub use llm_providers::web_search::WEB_SEARCH;
pub use lsp::Lsp;
pub(crate) use outside::settled;
pub(crate) use workspace::from_domain;
pub use workspace::{
    Confiner, Launch, Launcher, Models, Network, OnThisMachine, Policy, Verification, WebSearcher, Workspace,
    confinement_of, data_folders, env_secrets, temp_folders,
};

use crate::model::ToolSpec;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Outcome {
    pub content: String,
    pub is_error: bool,
    pub images: Vec<domain::Image>,
    #[serde(default)]
    pub shown: Option<String>,
    #[serde(default)]
    pub edited: Option<Box<domain::changes::Edited>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<Box<domain::events::BackgroundChange>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub carried_from: Option<Box<String>>,
}

impl Outcome {
    pub fn ok(content: impl Into<String>) -> Self {
        Self::new(content.into(), false, Vec::new())
    }

    fn new(content: String, is_error: bool, images: Vec<domain::Image>) -> Self {
        Self { content, is_error, images, shown: None, edited: None, background: None, carried_from: None }
    }

    pub fn carried_from(mut self, sources: Option<String>) -> Self {
        self.carried_from = sources.map(Box::new);
        self
    }

    pub fn in_the_background(mut self, change: domain::events::BackgroundChange) -> Self {
        self.background = Some(Box::new(change));
        self
    }

    pub fn showing(mut self, shown: impl Into<String>) -> Self {
        self.shown = Some(shown.into());
        self
    }

    pub fn editing(mut self, edited: domain::changes::Edited) -> Self {
        self.edited = Some(Box::new(edited));
        self
    }

    pub fn image(image: domain::Image) -> Self {
        Self::new(format!("Image ({})", image.media_type), false, vec![image])
    }

    pub fn error(content: impl Into<String>) -> Self {
        Self::new(content.into(), true, Vec::new())
    }
}

pub struct Consult<'a> {
    pub api: &'a dyn crate::model::ModelApi,
    pub request: &'a crate::model::TurnRequest,
    pub conversation: &'a [crate::transcript::Message],
    pub stop: &'a platform::Stop,
}

impl std::fmt::Debug for Consult<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Consult").field("model", &self.request.model).finish_non_exhaustive()
    }
}

pub trait Tool: Send + Sync {
    fn spec(&self) -> ToolSpec;

    fn effects(&self, input: &Value, workspace: &Workspace) -> Vec<Effect>;

    fn summary(&self, input: &Value) -> String;

    fn run(&self, input: &Value, workspace: &Workspace) -> Outcome;

    fn offered(&self, _workspace: &Workspace) -> bool {
        true
    }

    fn shown(&self, _input: &Value, _workspace: &Workspace) -> Option<String> {
        None
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

pub fn declared_path(name: &str) -> String {
    format!("tools/{name}.js")
}

pub fn declared(name: &str, data: &Value) -> crate::model::ToolSpec {
    let path = declared_path(name);
    let data = content::with_brand(data);
    let spec = content::value(&path, "spec").unwrap_or_default();
    let schema = match spec.get("input") {
        Some(input) => input.clone(),
        None => content::call(&path, "spec.input", &data).unwrap_or_else(|| serde_json::json!({ "type": "object" })),
    };
    crate::model::ToolSpec {
        name: name.to_owned(),
        description: content::render_export(&path, "spec.description", &data).unwrap_or_default(),
        schema,
    }
}

pub fn declares_execute(name: &str) -> bool {
    content::exports(&declared_path(name), "execute")
}

pub fn changes_nothing(tool: &dyn Tool, input: &Value, workspace: &Workspace) -> bool {
    domain::permission::change_nothing(&tool.effects(input, workspace))
}

pub(crate) fn writes(path: Option<&str>, workspace: &Workspace) -> Vec<Effect> {
    vec![path.map_or(Effect::Declared(Declared::Edits), |path| Effect::WritePath(workspace.place(path)))]
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
    use std::path::PathBuf;

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

#[cfg(test)]
mod declared_tests {
    use super::Tool;

    #[test]
    fn every_built_in_tool_is_declared_in_content_with_an_execute() {
        let mut tools: Vec<Box<dyn Tool>> = super::standard();
        tools.push(Box::new(super::Lsp));
        tools.push(Box::new(super::skill::SkillTool { skills: Vec::new() }));
        tools.push(Box::new(super::subagent::AgentTool { agents: Vec::new() }));
        tools.push(Box::new(super::send_message::SendMessageTool));
        tools.extend(super::task::tools());
        tools.push(Box::new(super::tool_search::ToolSearch));
        let schema = serde_json::json!({ "type": "string" });
        tools.push(Box::new(super::structured::Structured::new(schema.clone()).expect("structured")));
        for tool in &tools {
            let spec = tool.spec();
            let name = spec.name.as_str();
            assert!(content::builtin(&super::declared_path(name)).is_some(), "{name} has no content/tools file");
            assert!(!spec.description.trim().is_empty(), "{name} has no description");
            assert_eq!(spec.schema["type"], "object", "{name}");
            assert!(super::declares_execute(name), "{name} has no execute");
            assert!(!spec.description.contains("{{"), "{name} keeps a placeholder");
        }
        let declaring: Vec<&str> = content::builtin_files("tools/")
            .into_iter()
            .filter(|(_, source)| {
                String::from_utf8_lossy(source).lines().any(|line| line.trim_start().starts_with("access:"))
            })
            .map(|(path, _)| path)
            .collect();
        assert!(declaring.is_empty(), "a tool's effects are declared in Rust, not as `access` in {declaring:?}");
        let structured = tools.iter().find(|tool| tool.spec().name == "StructuredOutput").expect("structured");
        assert_eq!(structured.spec().schema["properties"]["result"], schema, "input is a function of the data");
    }
}
