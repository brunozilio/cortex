mod git;
mod instruction_files;
pub(crate) mod markdown;
pub(crate) mod permissions;
mod render;
pub use render::render;
pub mod rules;
pub mod settings;
pub mod styles;
pub const CONTRACT_VERSION: &str = "agent-engine/1";

use std::path::{Path, PathBuf};

use domain::PromptRequest;
pub use git::{GitSnapshot, snapshot};

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
pub struct Environment {
    pub cwd: PathBuf,
    pub added_dirs: Vec<PathBuf>,
    pub platform: &'static str,
    pub date: String,
    pub model: String,
    pub git: Option<GitSnapshot>,
    pub instructions: Vec<Instructions>,
    pub tools: Vec<String>,
    pub skills: String,
    pub output_style: Option<settings::Style>,
}

impl Environment {
    fn data(&self) -> serde_json::Value {
        let shown = |path: &Path| path.display().to_string();
        serde_json::json!({
            "cwd": shown(&self.cwd),
            "added_dirs": self.added_dirs.iter().map(|dir| shown(dir)).collect::<Vec<_>>(),
            "platform": self.platform,
            "date": self.date,
            "model": self.model,
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
            "contract": CONTRACT_VERSION,
            "brand": brand_data(),
        })
    }
}

fn brand_data() -> serde_json::Value {
    serde_json::json!({
        "NAME": brand::NAME, "COMMAND": brand::COMMAND, "DIR": brand::DIR,
        "INSTRUCTIONS": brand::INSTRUCTIONS, "ENV": brand::ENV,
    })
}

pub fn text(name: &str) -> String {
    content::text(&format!("prompts/{name}.md"))
}

pub fn system(environment: &Environment) -> String {
    render::render("prompts/system.js", &environment.data())
}

pub fn user_message(request: &PromptRequest) -> String {
    request.compose()
}

pub fn instructions(cwd: &Path, home: Option<&Path>) -> Vec<Instructions> {
    instruction_files::load(cwd, home)
}

pub const fn platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "macOS"
    } else if cfg!(windows) {
        "Windows"
    } else {
        "Linux"
    }
}

#[cfg(test)]
mod tests;

pub fn reminder(mode: domain::PermissionMode, depth: usize, style: Option<&str>) -> String {
    let mode = match mode {
        domain::PermissionMode::Plan => "plan",
        domain::PermissionMode::Ask => "ask",
        domain::PermissionMode::AcceptEdits => "edits",
        domain::PermissionMode::Bypass => "bypass",
    };
    let data = serde_json::json!({ "mode": mode, "depth": depth, "style": style, "brand": brand_data() });
    render::render("prompts/reminder.js", &data)
}
