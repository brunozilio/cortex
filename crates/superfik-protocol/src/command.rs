//! Requests a client sends to a backend.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use superfik_core::provider::ModelProvider;
use superfik_core::{AskId, Decision, MemoryEdit, PermissionMode, PromptRequest, RunId};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(clippy::large_enum_variant, reason = "commands are short-lived messages, never stored in bulk")]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum ClientCommand {
    SubmitPrompt {
        run: RunId,
        request: PromptRequest,
    },
    CancelRun {
        run: RunId,
    },
    AnswerAsk {
        run: RunId,
        ask: AskId,
        decision: Decision,
    },
    SetPermissionMode {
        run: RunId,
        mode: PermissionMode,
    },
    ListModels,
    ListSkills {
        path: PathBuf,
    },
    ListAgents {
        path: PathBuf,
    },
    LoadMemory {
        path: PathBuf,
    },
    SearchMemory {
        path: PathBuf,
        query: String,
    },
    EditMemory {
        path: PathBuf,
        edits: Vec<MemoryEdit>,
    },
    LoadSettings,
    SaveSettings {
        settings: serde_json::Value,
    },
    ListProviders,
    /// Connects with an API key, or a subscription's token JSON.
    ConnectProvider {
        provider: ModelProvider,
        key: String,
    },
    /// Signs in to a subscription in the browser; the backend answers with
    /// [`ProviderEvent::AuthorizeAt`](crate::ProviderEvent::AuthorizeAt).
    SignInProvider {
        provider: ModelProvider,
    },
    DisconnectProvider {
        provider: ModelProvider,
    },
}

impl ClientCommand {
    pub fn name(&self) -> &'static str {
        match self {
            Self::SubmitPrompt { .. } => "submit_prompt",
            Self::CancelRun { .. } => "cancel_run",
            Self::AnswerAsk { .. } => "answer_ask",
            Self::SetPermissionMode { .. } => "set_permission_mode",
            Self::ListModels => "list_models",
            Self::ListSkills { .. } => "list_skills",
            Self::ListAgents { .. } => "list_agents",
            Self::LoadMemory { .. } => "load_memory",
            Self::SearchMemory { .. } => "search_memory",
            Self::EditMemory { .. } => "edit_memory",
            Self::LoadSettings => "load_settings",
            Self::SaveSettings { .. } => "save_settings",
            Self::ListProviders => "list_providers",
            Self::ConnectProvider { .. } => "connect_provider",
            Self::SignInProvider { .. } => "sign_in_provider",
            Self::DisconnectProvider { .. } => "disconnect_provider",
        }
    }
}

#[cfg(test)]
mod tests {
    use superfik_core::ModelOption;

    use super::*;

    #[test]
    fn every_command_round_trips() {
        let run = RunId::new();
        let request = PromptRequest::plain("/repo".into(), "hi", ModelOption::new("m", "M"));
        let commands = [
            ClientCommand::SubmitPrompt { run, request },
            ClientCommand::CancelRun { run },
            ClientCommand::AnswerAsk { run, ask: AskId("a".into()), decision: Decision::Answer(vec![vec![0]]) },
            ClientCommand::SetPermissionMode { run, mode: PermissionMode::Plan },
            ClientCommand::ListModels,
            ClientCommand::ListSkills { path: "/repo".into() },
            ClientCommand::ListAgents { path: "/repo".into() },
            ClientCommand::LoadMemory { path: "/repo".into() },
            ClientCommand::SearchMemory { path: "/repo".into(), query: "q".into() },
            ClientCommand::EditMemory { path: "/repo".into(), edits: Vec::new() },
            ClientCommand::LoadSettings,
            ClientCommand::SaveSettings { settings: serde_json::json!({"model": "m"}) },
            ClientCommand::ListProviders,
            ClientCommand::ConnectProvider { provider: ModelProvider::Anthropic, key: "sk".into() },
            ClientCommand::SignInProvider { provider: ModelProvider::ClaudeSubscription },
            ClientCommand::DisconnectProvider { provider: ModelProvider::Anthropic },
        ];
        for command in commands {
            let json = serde_json::to_string(&command).expect("serialize");
            assert_eq!(serde_json::from_str::<ClientCommand>(&json).expect("deserialize"), command);
        }
    }
}
