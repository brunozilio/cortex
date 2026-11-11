use std::path::PathBuf;

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
    pub cwd: PathBuf,
    pub prompt: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<Image>,
    pub model: ModelOption,
    pub effort: Effort,
    pub resume: Option<SessionId>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub fork: bool,
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
    pub extensions: Option<Extensions>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub add_dirs: Vec<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_turns: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_tools: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub disallowed_tools: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub append_system: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification: Option<crate::verification::Criteria>,
    #[serde(skip)]
    pub credentials: crate::credentials::RunCredentials,
}

fn checked() -> bool {
    true
}

impl PromptRequest {
    pub fn plain(cwd: PathBuf, prompt: impl Into<String>, model: ModelOption) -> Self {
        Self {
            cwd,
            prompt: prompt.into(),
            images: Vec::new(),
            model,
            effort: Effort::Medium,
            resume: None,
            fork: false,
            command: None,
            memory: None,
            permission: PermissionMode::default(),
            output_style: None,
            safety_check: true,
            extensions: None,
            add_dirs: Vec::new(),
            max_turns: None,
            allowed_tools: Vec::new(),
            disallowed_tools: Vec::new(),
            fallback_model: None,
            append_system: None,
            verification: None,
            credentials: Default::default(),
        }
    }

    pub fn compose(&self) -> String {
        let mut out = self.prompt.trim().to_owned();
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

    #[test]
    fn the_typed_prompt_is_trimmed() {
        let request = PromptRequest::plain("/tmp".into(), "  Fix the bug. ", ModelOption::new("m", "M"));
        assert_eq!(request.compose(), "Fix the bug.");
    }

    #[test]
    fn a_request_round_trips_without_its_credentials() {
        let mut request = PromptRequest::plain("/tmp".into(), "hi", ModelOption::new("m", "M"));
        request.credentials.client = 7;
        let json = serde_json::to_string(&request).expect("serialize");
        let back: PromptRequest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.credentials.client, 0, "credentials never travel");
        assert_eq!(back.prompt, "hi");
    }
}
