//! What the user asks an engine to do.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::approval::PermissionMode;
use crate::context::{Reference, Skill};
use crate::ids::{SessionId, WorkspaceId};
use crate::memory::MemoryBrief;
use crate::model::{Effort, ModelOption};
use crate::plugins::{Connection, PluginBrief};
use crate::repository::Repository;
use crate::usage::SessionCommand;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromptRequest {
    pub workspace: WorkspaceId,
    pub cwd: PathBuf,
    pub prompt: String,
    pub model: ModelOption,
    pub effort: Effort,
    #[serde(default)]
    pub repositories: Vec<Repository>,
    pub skills: Vec<Skill>,
    #[serde(default)]
    pub disabled_skills: Vec<String>,
    pub references: Vec<Reference>,
    pub resume: Option<SessionId>,
    #[serde(default)]
    pub command: Option<SessionCommand>,
    #[serde(default)]
    pub memory: Option<MemoryBrief>,
    #[serde(default)]
    pub permission: PermissionMode,
    #[serde(default)]
    pub output_style: Option<String>,
    #[serde(default = "checked")]
    pub safety_check: bool,
    #[serde(default)]
    pub connections: Vec<Connection>,
    #[serde(default)]
    pub plugins: Option<PluginBrief>,
    #[serde(skip)]
    pub credentials: crate::credentials::RunCredentials,
}

fn checked() -> bool {
    true
}

impl PromptRequest {
    pub fn plain(cwd: PathBuf, prompt: impl Into<String>, model: ModelOption) -> Self {
        Self {
            workspace: WorkspaceId::new(),
            cwd,
            prompt: prompt.into(),
            model,
            effort: Effort::Medium,
            repositories: Vec::new(),
            skills: Vec::new(),
            disabled_skills: Vec::new(),
            references: Vec::new(),
            resume: None,
            command: None,
            memory: None,
            permission: PermissionMode::default(),
            output_style: None,
            safety_check: true,
            connections: Vec::new(),
            plugins: None,
            credentials: Default::default(),
        }
    }

    pub fn compose(&self) -> String {
        let mut out = self.prompt.trim().to_owned();
        if !self.repositories.is_empty() {
            out.push_str("\n\nYou also have these repositories to work in:");
            for repo in &self.repositories {
                out.push_str(&format!("\n- {} at {} (branch {})", repo.slug, repo.root.display(), repo.branch));
            }
        }
        if !self.references.is_empty() {
            out.push_str("\n\nReferences (read these files; images are visual references):");
            for reference in &self.references {
                out.push_str(&format!("\n- {}", reference.path.display()));
            }
        }
        if let Some(plugins) = self.plugins.as_ref().filter(|p| !p.instructions.is_empty()) {
            out.push_str("\n\nConnected accounts (use only their tools for each account):");
            for line in &plugins.instructions {
                out.push_str(&format!("\n- {line}"));
            }
        }
        if let Some(memory) = self.memory.as_ref().and_then(|memory| memory.instructions(self.resume.is_none())) {
            out.push_str("\n\n");
            out.push_str(&memory);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::EngineKind;

    fn request() -> PromptRequest {
        PromptRequest {
            workspace: WorkspaceId::new(),
            cwd: PathBuf::from("/tmp"),
            prompt: "  Create a small night sky scene. ".into(),
            model: ModelOption::new(EngineKind::Native, "claude-opus-5-5", "Opus 5.5"),
            effort: Effort::Medium,
            repositories: vec![],
            skills: vec![],
            disabled_skills: Vec::new(),
            references: vec![],
            resume: None,
            command: None,
            memory: None,
            permission: Default::default(),
            output_style: None,
            safety_check: true,
            connections: vec![],
            plugins: None,
            credentials: Default::default(),
        }
    }

    #[test]
    fn connected_accounts_are_listed() {
        let mut req = request();
        req.plugins = Some(PluginBrief {
            instructions: vec!["Gmail · work: tools mcp__gmail-work__*".into()],
            ..Default::default()
        });
        assert!(req.compose().ends_with(
            "Connected accounts (use only their tools for each account):\n- Gmail · work: tools mcp__gmail-work__*"
        ));
    }

    #[test]
    fn memory_is_recalled_only_into_a_fresh_session() {
        let mut req = request();
        let memory = crate::Memory {
            id: crate::MemoryId(4),
            kind: crate::MemoryKind::Semantic,
            text: "Uses wgpu".into(),
            tags: vec![],
            created: 0,
            origin: None,
            author: crate::Author::Agent,
        };
        req.memory = Some(MemoryBrief {
            recalled: vec![memory],
            output: "/tmp/m.json".into(),
            origin: None,
            tool: None,
            optimizing: false,
        });
        assert!(req.compose().contains("- [4] Uses wgpu"));
        req.resume = Some(SessionId("s".into()));
        let resumed = req.compose();
        assert!(!resumed.contains("[4]") && resumed.contains("/tmp/m.json"));
    }

    #[test]
    fn bare_prompt_is_trimmed() {
        assert_eq!(request().compose(), "Create a small night sky scene.");
    }

    #[test]
    fn context_is_appended() {
        let mut req = request();
        req.repositories.push(Repository {
            root: "/code/engine".into(),
            slug: "acme/engine".into(),
            branch: "dev".into(),
            ..Repository::default()
        });
        let composed = req.compose();
        assert!(composed.contains("- acme/engine at /code/engine (branch dev)"));
        assert!(!composed.contains("sub-agents"), "the agent decides on sub-agents itself");
    }
}
