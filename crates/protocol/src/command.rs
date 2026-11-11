use std::path::PathBuf;

use domain::provider::ModelProvider;
use domain::{AskId, Decision, PermissionMode, PromptRequest, RunId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(clippy::large_enum_variant, reason = "commands are short-lived messages, never stored in bulk")]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum ClientCommand {
    SubmitPrompt { run: RunId, request: PromptRequest },
    CancelRun { run: RunId },
    AnswerAsk { run: RunId, ask: AskId, decision: Decision },
    SetPermissionMode { run: RunId, mode: PermissionMode },
    SteerRun { run: RunId, text: String },
    UnsteerRun { run: RunId },
    BackgroundRun { run: RunId },
    ListModels,
    ListSkills { path: PathBuf },
    ListAgents { path: PathBuf },
    LoadSettings,
    SaveSettings { settings: serde_json::Value },
    ListProviders,
    ConnectProvider { provider: ModelProvider, key: String },
    SignInProvider { provider: ModelProvider },
    DisconnectProvider { provider: ModelProvider },
    ListMcpServers { path: PathBuf },
    TakeNotices,
    Feature { feature: String, input: serde_json::Value },
    ListPermissions { path: PathBuf },
    ApproveMcpServer { path: PathBuf, name: String, approve: bool },
    AddMcpServer { path: PathBuf, name: String, server: serde_json::Value },
    RemoveMcpServer { path: PathBuf, name: String },
    SignInMcpServer { path: PathBuf, name: String },
    SignOutMcpServer { path: PathBuf, name: String },
    OpenChannels { path: PathBuf, names: Vec<String> },
}

impl ClientCommand {
    pub fn name(&self) -> &'static str {
        match self {
            Self::SubmitPrompt { .. } => "submit_prompt",
            Self::CancelRun { .. } => "cancel_run",
            Self::AnswerAsk { .. } => "answer_ask",
            Self::SetPermissionMode { .. } => "set_permission_mode",
            Self::SteerRun { .. } => "steer_run",
            Self::UnsteerRun { .. } => "unsteer_run",
            Self::BackgroundRun { .. } => "background_run",
            Self::ListModels => "list_models",
            Self::ListSkills { .. } => "list_skills",
            Self::ListAgents { .. } => "list_agents",
            Self::LoadSettings => "load_settings",
            Self::SaveSettings { .. } => "save_settings",
            Self::ListProviders => "list_providers",
            Self::ConnectProvider { .. } => "connect_provider",
            Self::SignInProvider { .. } => "sign_in_provider",
            Self::DisconnectProvider { .. } => "disconnect_provider",
            Self::ListMcpServers { .. } => "list_mcp_servers",
            Self::ListPermissions { .. } => "list_permissions",
            Self::TakeNotices => "take_notices",
            Self::Feature { .. } => "feature",
            Self::ApproveMcpServer { .. } => "approve_mcp_server",
            Self::AddMcpServer { .. } => "add_mcp_server",
            Self::RemoveMcpServer { .. } => "remove_mcp_server",
            Self::SignInMcpServer { .. } => "sign_in_mcp_server",
            Self::SignOutMcpServer { .. } => "sign_out_mcp_server",
            Self::OpenChannels { .. } => "open_channels",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::ModelOption;

    #[test]
    fn every_command_round_trips() {
        let run = RunId::new();
        let request = PromptRequest::plain("/repo".into(), "hi", ModelOption::new("m", "M"));
        let commands = [
            ClientCommand::SubmitPrompt { run, request },
            ClientCommand::CancelRun { run },
            ClientCommand::AnswerAsk { run, ask: AskId("a".into()), decision: Decision::Answer(vec![vec![0]]) },
            ClientCommand::SetPermissionMode { run, mode: PermissionMode::Plan },
            ClientCommand::SteerRun { run, text: "use tabs".into() },
            ClientCommand::UnsteerRun { run },
            ClientCommand::BackgroundRun { run },
            ClientCommand::ListModels,
            ClientCommand::ListSkills { path: "/repo".into() },
            ClientCommand::ListAgents { path: "/repo".into() },
            ClientCommand::LoadSettings,
            ClientCommand::SaveSettings { settings: serde_json::json!({"model": "m"}) },
            ClientCommand::ListProviders,
            ClientCommand::ConnectProvider { provider: ModelProvider::new("anthropic"), key: "sk".into() },
            ClientCommand::SignInProvider { provider: ModelProvider::new("claude-subscription") },
            ClientCommand::DisconnectProvider { provider: ModelProvider::new("anthropic") },
            ClientCommand::ListMcpServers { path: "/repo".into() },
            ClientCommand::ListPermissions { path: "/repo".into() },
            ClientCommand::TakeNotices,
            ClientCommand::Feature { feature: "schedules".into(), input: serde_json::json!({"delete": "cron_1"}) },
            ClientCommand::ApproveMcpServer { path: "/repo".into(), name: "jira".into(), approve: true },
            ClientCommand::AddMcpServer {
                path: "/repo".into(),
                name: "atlassian".into(),
                server: serde_json::json!({"type": "http", "url": "https://mcp.atlassian.com/v1/mcp"}),
            },
            ClientCommand::RemoveMcpServer { path: "/repo".into(), name: "atlassian".into() },
            ClientCommand::SignInMcpServer { path: "/repo".into(), name: "linear".into() },
            ClientCommand::SignOutMcpServer { path: "/repo".into(), name: "linear".into() },
            ClientCommand::OpenChannels { path: "/repo".into(), names: vec!["telegram".into()] },
        ];
        for command in commands {
            let json = serde_json::to_string(&command).expect("serialize");
            assert_eq!(serde_json::from_str::<ClientCommand>(&json).expect("deserialize"), command);
        }
    }
}
