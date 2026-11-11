use std::path::PathBuf;

use domain::provider::ModelProvider;
use domain::{AskId, Decision, MemoryEdit, ModelOption, PermissionMode, PromptRequest, RunId};
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
    LoadMemory { path: PathBuf },
    SearchMemory { path: PathBuf, query: String },
    EditMemory { path: PathBuf, edits: Vec<MemoryEdit> },
    OptimizeMemory { run: RunId, path: PathBuf, model: ModelOption },
    UndoMemory { path: PathBuf },
    LoadSettings,
    SaveSettings { settings: serde_json::Value },
    ListProviders,
    ConnectProvider { provider: ModelProvider, key: String },
    SignInProvider { provider: ModelProvider },
    DisconnectProvider { provider: ModelProvider },
    ListMcpServers { path: PathBuf },
    ListShells { kill: Option<String> },
    Schedules { delete: Option<String>, take_due: bool },
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
            Self::LoadMemory { .. } => "load_memory",
            Self::SearchMemory { .. } => "search_memory",
            Self::EditMemory { .. } => "edit_memory",
            Self::OptimizeMemory { .. } => "optimize_memory",
            Self::UndoMemory { .. } => "undo_memory",
            Self::LoadSettings => "load_settings",
            Self::SaveSettings { .. } => "save_settings",
            Self::ListProviders => "list_providers",
            Self::ConnectProvider { .. } => "connect_provider",
            Self::SignInProvider { .. } => "sign_in_provider",
            Self::DisconnectProvider { .. } => "disconnect_provider",
            Self::ListMcpServers { .. } => "list_mcp_servers",
            Self::ListPermissions { .. } => "list_permissions",
            Self::ListShells { .. } => "list_shells",
            Self::Schedules { .. } => "schedules",
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
            ClientCommand::LoadMemory { path: "/repo".into() },
            ClientCommand::SearchMemory { path: "/repo".into(), query: "q".into() },
            ClientCommand::EditMemory { path: "/repo".into(), edits: Vec::new() },
            ClientCommand::OptimizeMemory { run, path: "/repo".into(), model: ModelOption::new("m", "M") },
            ClientCommand::UndoMemory { path: "/repo".into() },
            ClientCommand::LoadSettings,
            ClientCommand::SaveSettings { settings: serde_json::json!({"model": "m"}) },
            ClientCommand::ListProviders,
            ClientCommand::ConnectProvider { provider: ModelProvider::new("anthropic"), key: "sk".into() },
            ClientCommand::SignInProvider { provider: ModelProvider::new("claude-subscription") },
            ClientCommand::DisconnectProvider { provider: ModelProvider::new("anthropic") },
            ClientCommand::ListMcpServers { path: "/repo".into() },
            ClientCommand::ListPermissions { path: "/repo".into() },
            ClientCommand::ListShells { kill: Some("bash_1".into()) },
            ClientCommand::Schedules { delete: Some("cron_1".into()), take_due: true },
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
