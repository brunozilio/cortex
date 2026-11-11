mod git;
mod instruction_files;
pub(crate) mod markdown;
pub(crate) mod permissions;
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
    fn has(&self, prefix: &str) -> bool {
        self.tools.iter().any(|tool| tool.starts_with(prefix))
    }
}

pub fn text(name: &str) -> String {
    content::text(&format!("prompts/{name}.md"))
}

pub fn system(environment: &Environment) -> String {
    let mut sections = vec![text("role"), text("harness")];
    if environment.has(concat!("mcp__", brand::command!(), "_memory__")) {
        sections.push(text("memory"));
    }
    sections.push(text("context"));
    if environment.output_style.as_ref().is_none_or(|style| style.keep_coding) {
        sections.extend([text("delivery"), text("writing")]);
    }
    if environment.tools.iter().any(|tool| tool == crate::tools::advisor::ADVISOR_TOOL) {
        sections.push(text("advisor"));
    }
    let mut prompt = sections.join("\n\n");
    prompt.push_str(&format!("\n\nPrompt contract: {CONTRACT_VERSION}"));
    if let Some(style) = &environment.output_style {
        prompt.push_str(&format!("\n\n# Output style: {}\n{}", style.name, style.instructions));
    }
    if !environment.skills.is_empty() {
        prompt.push_str("\n\n");
        prompt.push_str(&text("skills"));
        prompt.push('\n');
        prompt.push_str(&environment.skills);
    }
    if !environment.instructions.is_empty() {
        prompt.push_str("\n\n");
        prompt.push_str(&text("instructions"));
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
    if !environment.added_dirs.is_empty() {
        let dirs: Vec<String> = environment.added_dirs.iter().map(|dir| dir.display().to_string()).collect();
        prompt.push_str(&format!("- Additional working directories: {}\n", dirs.join(", ")));
    }
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

pub fn reminder(mode: domain::PermissionMode, depth: usize, style: Option<&str>) -> String {
    let mode = match mode {
        domain::PermissionMode::Plan if depth == 0 => text("mode-plan"),
        domain::PermissionMode::Plan => text("mode-plan-sub-agent"),
        domain::PermissionMode::Ask => text("mode-ask"),
        domain::PermissionMode::AcceptEdits => text("mode-edits"),
        domain::PermissionMode::Bypass => text("mode-bypass"),
    };
    let role = if depth > 0 { text("reminder-sub-agent") } else { text("reminder-main") };
    let mut reminder = format!("\n\n# Runtime reminder\n{mode}\n{role}");
    if let Some(style) = style.filter(|_| depth == 0) {
        reminder.push('\n');
        reminder.push_str(style);
    }
    reminder
}
