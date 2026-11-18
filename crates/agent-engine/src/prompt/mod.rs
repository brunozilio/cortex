mod criteria_file;
mod git;
mod instruction_files;
pub(crate) mod markdown;
pub mod owner_settings;
pub mod permissions;
pub mod rules;
pub mod settings;
pub mod styles;
mod top_level;
pub const CONTRACT_VERSION: &str = "agent-engine/1";

use std::path::{Path, PathBuf};

use domain::PromptRequest;
pub use git::{GitSnapshot, Snapshot, snapshot};
pub use top_level::top_level;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Scope {
    User,
    Project,
    Local,
}

impl Scope {
    const fn id(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Project => "project",
            Self::Local => "local",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Instructions {
    pub path: PathBuf,
    pub scope: Scope,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LanguageServer {
    pub name: String,
    pub extensions: Vec<String>,
}

impl LanguageServer {
    pub fn of(spec: &domain::lsp::ServerSpec) -> Self {
        Self { name: spec.name.clone(), extensions: spec.extension_to_language.keys().cloned().collect() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Environment {
    pub cwd: PathBuf,
    pub added_dirs: Vec<PathBuf>,
    pub platform: &'static str,
    pub date: String,
    pub time_zone: String,
    pub top_level: Option<Vec<String>>,
    pub model: String,
    pub language_servers: Vec<LanguageServer>,
    pub git: Option<GitSnapshot>,
    pub instructions: Vec<Instructions>,
    pub tools: Vec<String>,
    pub skills: String,
    pub output_style: Option<settings::Style>,
    pub mcp_instructions: Vec<McpInstructions>,
}

pub const MCP_INSTRUCTIONS: usize = 2_048;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpInstructions {
    pub server: String,
    pub text: String,
}

impl McpInstructions {
    pub fn of(server: &str, text: &str) -> Self {
        let text = text.trim();
        let text = match text.char_indices().nth(MCP_INSTRUCTIONS) {
            Some((at, _)) => format!("{}… [cut]", &text[..at]),
            None => text.to_owned(),
        };
        Self { server: server.to_owned(), text }
    }
}

impl Environment {
    fn data(&self) -> serde_json::Value {
        let shown = |path: &Path| path.display().to_string();
        serde_json::json!({
            "cwd": shown(&self.cwd),
            "added_dirs": self.added_dirs.iter().map(|dir| shown(dir)).collect::<Vec<_>>(),
            "platform": self.platform,
            "date": self.date,
            "time_zone": self.time_zone,
            "top_level": self.top_level,
            "model": self.model,
            "language_servers": self.language_servers.iter().map(|server| serde_json::json!({
                "name": server.name, "extensions": server.extensions,
            })).collect::<Vec<_>>(),
            "git": self.git.as_ref().map(|git| serde_json::json!({
                "branch": git.branch, "status": git.status, "recent": git.recent,
            })),
            "instructions": self.instructions.iter().map(|file| serde_json::json!({
                "path": shown(&file.path), "scope": file.scope.id(), "text": file.text,
            })).collect::<Vec<_>>(),
            "tools": self.tools,
            "skills": self.skills,
            "output_style": self.output_style.as_ref().map(|style| serde_json::json!({
                "name": style.name, "instructions": style.instructions, "keep_coding": style.keep_coding,
            })),
            "mcp_instructions": self.mcp_instructions.iter().map(|served| {
                serde_json::json!({ "server": served.server, "text": McpInstructions::of(&served.server, &served.text).text })
            }).collect::<Vec<_>>(),
            "contract": CONTRACT_VERSION,
            "brand": content::brand(),
        })
    }
}

pub fn render(path: &str, data: &serde_json::Value) -> String {
    content::render(path, &content::with_brand(data))
}

pub fn text(name: &str) -> String {
    content::text(&format!("prompts/{name}.js"))
}

pub fn system(environment: &Environment) -> String {
    render("prompts/system.js", &environment.data())
}

pub fn user_message(request: &PromptRequest) -> String {
    request.compose()
}

pub fn instructions(cwd: &Path, home: Option<&Path>) -> Vec<Instructions> {
    instruction_files::load(cwd, home)
}

pub const fn platform() -> &'static str {
    platform::NAME
}

#[cfg(test)]
mod tests;

pub fn reminder(mode: domain::PermissionMode, depth: usize, style: Option<&str>) -> String {
    let mode = domain::command::mode_label(mode);
    let data = serde_json::json!({ "mode": mode, "depth": depth, "style": style });
    render("prompts/reminder.js", &data)
}
