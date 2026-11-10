//! The native engine's system prompt (ADR 0015): Superfik's own text
//! ([`text`]), the sections for the tools the run was granted, the
//! instruction files (`SUPERFIK.md` first) and where the agent runs.

mod git;
mod instruction_files;
pub(crate) mod markdown;
pub mod rules;
pub mod settings;
pub mod styles;
mod text;
pub const CONTRACT_VERSION: &str = "superfik-native/1";

use std::path::{Path, PathBuf};

pub use git::{GitSnapshot, snapshot};
use superfik_core::PromptRequest;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Scope {
    User,
    Project,
    Local,
}

impl Scope {
    const fn label(self) -> &'static str {
        match self {
            Self::User => "the user's private instructions for every project",
            Self::Project => "the project's instructions, kept in the repository",
            Self::Local => "the user's private instructions for this project",
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
    fn has(&self, prefix: &str) -> bool {
        self.tools.iter().any(|tool| tool.starts_with(prefix))
    }
}

pub fn system(environment: &Environment) -> String {
    let mut sections = vec![text::ROLE, text::HARNESS];
    if environment.has("mcp__superfik_memory__") {
        sections.push(text::MEMORY);
    }
    sections.push(text::CONTEXT);
    if environment.output_style.as_ref().is_none_or(|style| style.keep_coding) {
        sections.extend([text::DELIVERY, text::WRITING]);
    }
    if environment.tools.iter().any(|tool| tool == crate::tools::advisor::ADVISOR_TOOL) {
        sections.push(crate::tools::advisor::GUIDE);
    }
    let mut prompt = sections.join("\n\n");
    prompt.push_str(&format!("\n\nPrompt contract: {CONTRACT_VERSION}"));
    if let Some(style) = &environment.output_style {
        prompt.push_str(&format!("\n\n# Output style: {}\n{}", style.name, style.instructions));
    }
    if !environment.skills.is_empty() {
        prompt.push_str("\n\n");
        prompt.push_str(text::SKILLS);
        prompt.push_str(&environment.skills);
    }
    if !environment.instructions.is_empty() {
        prompt.push_str("\n\n");
        prompt.push_str(text::INSTRUCTIONS);
        for file in &environment.instructions {
            prompt.push_str(&format!(
                "\n\nContents of {} ({}):\n\n{}",
                file.path.display(),
                file.scope.label(),
                file.text.trim()
            ));
        }
    }
    prompt.push_str("\n\n# Environment\n");
    prompt.push_str(&format!("- Working directory: {}\n", environment.cwd.display()));
    prompt.push_str(&format!("- Git repository: {}\n", if environment.git.is_some() { "yes" } else { "no" }));
    prompt.push_str(&format!("- Platform: {}\n", environment.platform));
    prompt.push_str(&format!("- Today: {}\n", environment.date));
    if !environment.model.is_empty() {
        prompt.push_str(&format!("- Model: {}\n", environment.model));
    }
    if let Some(git) = &environment.git {
        prompt.push_str(&git.describe());
    }
    prompt
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

pub fn reminder(mode: superfik_core::PermissionMode, depth: usize, style: Option<&str>) -> String {
    let mode = match mode {
        superfik_core::PermissionMode::Plan if depth == 0 => text::PLAN_MODE,
        superfik_core::PermissionMode::Plan => {
            "Planning mode: only read/search tools are available. Report what you found; do not edit files, run commands or change external state."
        }
        superfik_core::PermissionMode::Ask => {
            "Ask mode: tool actions require the permissions enforced by the runtime. Respect denials and do not bypass them."
        }
        superfik_core::PermissionMode::AcceptEdits => {
            "Edits are allowed; commands and external actions still require approval."
        }
        superfik_core::PermissionMode::Bypass => {
            "The user enabled autonomous execution. Stay within the authorized task and verify the result."
        }
    };
    let mut reminder = format!(
        "\n\n# Runtime reminder\n{mode}\n{}",
        if depth > 0 {
            "You are a sub-agent; return your report and do not create other agents."
        } else {
            "Collect background agent reports before completing. Use TaskStop to cancel an unnecessary task."
        }
    );
    if let Some(style) = style.filter(|_| depth == 0) {
        reminder.push('\n');
        reminder.push_str(style);
    }
    reminder
}
