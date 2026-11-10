//! What the user asks an engine to do.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::approval::PermissionMode;
use crate::extensions::Extensions;
use crate::ids::SessionId;
use crate::memory::MemoryBrief;
use crate::model::{Effort, ModelOption};
use crate::usage::SessionCommand;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromptRequest {
    pub cwd: PathBuf,
    pub prompt: String,
    pub model: ModelOption,
    pub effort: Effort,
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
    pub extensions: Option<Extensions>,
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
            model,
            effort: Effort::Medium,
            resume: None,
            command: None,
            memory: None,
            permission: PermissionMode::default(),
            output_style: None,
            safety_check: true,
            extensions: None,
            credentials: Default::default(),
        }
    }

    /// The prompt as the model reads it: what was typed, then the memory's
    /// instructions on a fresh conversation.
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
