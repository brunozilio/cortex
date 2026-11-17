use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::approval::PermissionMode;
use crate::extensions::Extensions;
use crate::ids::SessionId;
use crate::image::Image;
use crate::memory::MemoryBrief;
use crate::model::{Effort, ModelOption};
use crate::usage::SessionCommand;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromptRequest {
    #[serde(flatten)]
    pub prompt: Prompt,
    #[serde(flatten)]
    pub session: SessionConfig,
    #[serde(flatten)]
    pub policy: Policy,
    #[serde(flatten)]
    pub host: HostContext,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Prompt {
    #[serde(rename = "prompt")]
    pub text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<Image>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shown: Option<Shown>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionConfig {
    pub cwd: PathBuf,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub add_dirs: Vec<PathBuf>,
    pub model: ModelOption,
    pub effort: Effort,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub auto_pick: bool,
    pub resume: Option<SessionId>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub fork: bool,
    #[serde(default)]
    pub command: Option<SessionCommand>,
    #[serde(default)]
    pub output_style: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback_model: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fallbacks: Vec<String>,
    #[serde(default, skip_serializing_if = "crate::model_roles::ModelRoles::is_empty")]
    pub roles: crate::model_roles::ModelRoles,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub append_system: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Policy {
    #[serde(default)]
    pub permission: PermissionMode,
    #[serde(default = "checked")]
    pub safety_check: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_turns: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_tools: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub disallowed_tools: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ask_tools: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification: Option<crate::verification::Criteria>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget: Option<crate::budget::Budget>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct HostContext {
    #[serde(default)]
    pub memory: Option<MemoryBrief>,
    #[serde(default)]
    pub extensions: Option<Extensions>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code_map: Option<String>,
    #[serde(skip)]
    pub credentials: crate::credentials::RunCredentials,
}

#[derive(Debug, Clone, Copy)]
pub struct Folder<'a> {
    pub cwd: &'a Path,
    pub extensions: Option<&'a Extensions>,
}

impl<'a> Folder<'a> {
    pub fn at(cwd: &'a Path, extensions: &'a Extensions) -> Self {
        Self { cwd, extensions: Some(extensions) }
    }

    pub fn home(self) -> Option<&'a Path> {
        self.extensions.and_then(|extensions| extensions.home.as_deref())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shown {
    pub text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attached: Vec<String>,
}

fn checked() -> bool {
    true
}

impl Prompt {
    pub fn typed(text: impl Into<String>) -> Self {
        Self { text: text.into(), ..Self::default() }
    }
}

impl SessionConfig {
    pub fn in_folder(cwd: PathBuf, model: ModelOption) -> Self {
        Self {
            cwd,
            add_dirs: Vec::new(),
            model,
            effort: Effort::Medium,
            auto_pick: false,
            resume: None,
            fork: false,
            command: None,
            output_style: None,
            fallback_model: None,
            fallbacks: Vec::new(),
            roles: Default::default(),
            append_system: None,
        }
    }
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            permission: PermissionMode::default(),
            safety_check: true,
            max_turns: None,
            allowed_tools: Vec::new(),
            disallowed_tools: Vec::new(),
            ask_tools: Vec::new(),
            verification: None,
            budget: None,
        }
    }
}

impl PromptRequest {
    pub fn plain(cwd: PathBuf, prompt: impl Into<String>, model: ModelOption) -> Self {
        Self {
            prompt: Prompt::typed(prompt),
            session: SessionConfig::in_folder(cwd, model),
            policy: Policy::default(),
            host: HostContext::default(),
        }
    }

    pub fn remembering(mut self, memory: MemoryBrief) -> Self {
        self.host.memory = Some(memory);
        self
    }

    pub fn folder(&self) -> Folder<'_> {
        Folder { cwd: &self.session.cwd, extensions: self.host.extensions.as_ref() }
    }

    pub fn compose(&self) -> String {
        self.composed(&self.prompt.text)
    }

    pub fn composed(&self, typed: &str) -> String {
        let mut out = typed.trim().to_owned();
        if let Some(map) = self.host.code_map.as_deref().map(str::trim).filter(|map| !map.is_empty()) {
            out.push_str("\n\n");
            out.push_str(map);
        }
        let first_run = self.session.resume.is_none();
        if let Some(memory) = self.host.memory.as_ref().and_then(|memory| memory.recall_notes(first_run)) {
            out.push_str("\n\n");
            out.push_str(&memory);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_typed_prompt_is_trimmed() {
        let request = PromptRequest::plain("/tmp".into(), "  Fix the bug. ", ModelOption::new("m", "M"));
        assert_eq!(request.compose(), "Fix the bug.");
    }

    #[test]
    fn the_code_map_follows_the_typed_prompt() {
        let mut request = PromptRequest::plain("/tmp".into(), "Fix retry", ModelOption::new("m", "M"));
        request.host.code_map = Some("<code-map>\n- src/a.rs: fn retry (L3)\n</code-map>\n".into());
        assert_eq!(request.compose(), "Fix retry\n\n<code-map>\n- src/a.rs: fn retry (L3)\n</code-map>");
        request.host.code_map = Some("  ".into());
        assert_eq!(request.compose(), "Fix retry");
    }

    #[test]
    fn a_request_round_trips_without_its_credentials() {
        let mut request = PromptRequest::plain("/tmp".into(), "hi", ModelOption::new("m", "M"));
        request.host.credentials.client = 7;
        let json = serde_json::to_string(&request).expect("serialize");
        let back: PromptRequest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.host.credentials.client, 0, "credentials never travel");
        assert_eq!(back.prompt.text, "hi");
    }

    #[test]
    fn the_four_parts_are_written_as_one_flat_object_with_the_names_it_always_had() {
        let mut request = PromptRequest::plain("/tmp".into(), "hi", ModelOption::new("m", "M"));
        request.session.add_dirs = vec!["/lib".into()];
        request.session.fork = true;
        request.session.fallbacks = vec!["small".into()];
        request.policy.max_turns = Some(3);
        request.policy.allowed_tools = vec!["Read".into()];
        request.host.code_map = Some("map".into());
        let written = serde_json::to_value(&request).expect("written");
        let mut keys: Vec<&str> = written.as_object().expect("object").keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "add_dirs",
                "allowed_tools",
                "code_map",
                "command",
                "cwd",
                "effort",
                "extensions",
                "fallbacks",
                "fork",
                "max_turns",
                "memory",
                "model",
                "output_style",
                "permission",
                "prompt",
                "resume",
                "safety_check"
            ]
        );
        assert_eq!(written["prompt"], "hi");
        let old = serde_json::json!({
            "cwd": "/tmp", "prompt": "hi", "model": written["model"], "effort": written["effort"], "resume": null,
            "max_turns": 3
        });
        let read: PromptRequest = serde_json::from_value(old).expect("an older request reads");
        assert_eq!((read.policy.max_turns, read.policy.safety_check), (Some(3), true));
    }
}
