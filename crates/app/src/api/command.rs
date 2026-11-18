use std::path::PathBuf;

use domain::provider::ModelProvider;
use domain::{AskId, Decision, PermissionMode, PromptRequest, RunId};
use serde::{Deserialize, Serialize};

use crate::api::{FeatureRequest, RequestId};

pub const SIGN_IN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);

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
    SteerRun {
        run: RunId,
        text: String,
    },
    UnsteerRun {
        run: RunId,
    },
    BackgroundRun {
        run: RunId,
    },
    ListModels,
    RegisterPeer {
        path: PathBuf,
    },
    ListSkills {
        path: PathBuf,
    },
    ListAgents {
        path: PathBuf,
    },
    LoadSettings,
    SaveSettings {
        settings: serde_json::Value,
    },
    ChangeSettings {
        changes: Vec<crate::settings::SettingsChange>,
    },
    ListProviders,
    ConnectProvider {
        provider: ModelProvider,
        key: String,
    },
    SignInProvider {
        provider: ModelProvider,
    },
    CloseAndSignIn {
        provider: ModelProvider,
        port: u16,
        pid: u32,
    },
    DisconnectProvider {
        provider: ModelProvider,
    },
    UseProviderAccount {
        provider: ModelProvider,
        account: String,
    },
    RemoveProviderAccount {
        provider: ModelProvider,
        account: String,
    },
    SetProviderBalance {
        provider: ModelProvider,
        balance: domain::account_pool::Balance,
    },
    ListMcpServers {
        path: PathBuf,
    },
    TakeNotices {
        for_agent: bool,
    },
    AnswerTeammate {
        ask: domain::team::TeammateAsk,
        decision: domain::Decision,
    },
    Feature(FeatureRequest),
    ListPermissions {
        path: PathBuf,
    },
    ApproveMcpServer {
        path: PathBuf,
        name: String,
        approve: bool,
    },
    AddMcpServer {
        path: PathBuf,
        name: String,
        server: serde_json::Value,
        #[serde(default)]
        replace: bool,
    },
    GetMcpServer {
        path: PathBuf,
        name: String,
    },
    RemoveMcpServer {
        path: PathBuf,
        name: String,
    },
    SignInMcpServer {
        path: PathBuf,
        name: String,
    },
    SignOutMcpServer {
        path: PathBuf,
        name: String,
    },
    Trust {
        path: PathBuf,
        trusted: bool,
    },
    TrustForThisProcess {
        path: PathBuf,
    },
    TrustState {
        path: PathBuf,
    },
    CancelRequest {
        request: RequestId,
    },
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
            Self::RegisterPeer { .. } => "register_peer",
            Self::ListSkills { .. } => "list_skills",
            Self::ListAgents { .. } => "list_agents",
            Self::LoadSettings => "load_settings",
            Self::SaveSettings { .. } => "save_settings",
            Self::ChangeSettings { .. } => "change_settings",
            Self::ListProviders => "list_providers",
            Self::ConnectProvider { .. } => "connect_provider",
            Self::SignInProvider { .. } => "sign_in_provider",
            Self::CloseAndSignIn { .. } => "close_and_sign_in",
            Self::DisconnectProvider { .. } => "disconnect_provider",
            Self::UseProviderAccount { .. } => "use_provider_account",
            Self::RemoveProviderAccount { .. } => "remove_provider_account",
            Self::SetProviderBalance { .. } => "set_provider_balance",
            Self::ListMcpServers { .. } => "list_mcp_servers",
            Self::ListPermissions { .. } => "list_permissions",
            Self::TakeNotices { .. } => "take_notices",
            Self::AnswerTeammate { .. } => "answer_teammate",
            Self::Feature(_) => "feature",
            Self::ApproveMcpServer { .. } => "approve_mcp_server",
            Self::AddMcpServer { .. } => "add_mcp_server",
            Self::GetMcpServer { .. } => "get_mcp_server",
            Self::RemoveMcpServer { .. } => "remove_mcp_server",
            Self::SignInMcpServer { .. } => "sign_in_mcp_server",
            Self::SignOutMcpServer { .. } => "sign_out_mcp_server",
            Self::Trust { .. } => "trust",
            Self::TrustForThisProcess { .. } => "trust_for_this_process",
            Self::TrustState { .. } => "trust_state",
            Self::CancelRequest { .. } => "cancel_request",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::features::schedules::SchedulesRequest;
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
            ClientCommand::RegisterPeer { path: "/repo".into() },
            ClientCommand::ListSkills { path: "/repo".into() },
            ClientCommand::ListAgents { path: "/repo".into() },
            ClientCommand::LoadSettings,
            ClientCommand::SaveSettings { settings: serde_json::json!({"model": "m"}) },
            ClientCommand::ChangeSettings {
                changes: vec![
                    crate::settings::SettingsChange::set(["model"], serde_json::json!("m")),
                    crate::settings::SettingsChange::Remove { path: vec!["mode".into()] },
                ],
            },
            ClientCommand::ListProviders,
            ClientCommand::ConnectProvider { provider: ModelProvider::new("keyed"), key: "sk".into() },
            ClientCommand::SignInProvider { provider: ModelProvider::new("browser-sub") },
            ClientCommand::CloseAndSignIn { provider: ModelProvider::new("browser-sub"), port: 49153, pid: 998 },
            ClientCommand::DisconnectProvider { provider: ModelProvider::new("keyed") },
            ClientCommand::UseProviderAccount { provider: ModelProvider::new("keyed"), account: "2".into() },
            ClientCommand::RemoveProviderAccount { provider: ModelProvider::new("keyed"), account: "a@x.io".into() },
            ClientCommand::SetProviderBalance {
                provider: ModelProvider::new("keyed"),
                balance: domain::account_pool::Balance::RoundRobin,
            },
            ClientCommand::ListMcpServers { path: "/repo".into() },
            ClientCommand::ListPermissions { path: "/repo".into() },
            ClientCommand::TakeNotices { for_agent: true },
            ClientCommand::AnswerTeammate {
                ask: domain::team::TeammateAsk {
                    team: "team-1".into(),
                    teammate: "tester".into(),
                    request_id: "p1".into(),
                    asked: domain::team::Asked::Permission {
                        tool_name: "Bash".into(),
                        description: "npm test".into(),
                        rememberable: true,
                        rule: None,
                    },
                },
                decision: domain::Decision::Allow,
            },
            ClientCommand::Feature(FeatureRequest::Schedules(SchedulesRequest::Delete { id: "cron_1".into() })),
            ClientCommand::Feature(FeatureRequest::OutputSavings),
            ClientCommand::ApproveMcpServer { path: "/repo".into(), name: "jira".into(), approve: true },
            ClientCommand::AddMcpServer {
                path: "/repo".into(),
                name: "atlassian".into(),
                server: serde_json::json!({"type": "http", "url": "https://mcp.atlassian.com/v1/mcp"}),
                replace: true,
            },
            ClientCommand::GetMcpServer { path: "/repo".into(), name: "atlassian".into() },
            ClientCommand::RemoveMcpServer { path: "/repo".into(), name: "atlassian".into() },
            ClientCommand::SignInMcpServer { path: "/repo".into(), name: "linear".into() },
            ClientCommand::SignOutMcpServer { path: "/repo".into(), name: "linear".into() },
            ClientCommand::Trust { path: "/repo".into(), trusted: true },
            ClientCommand::TrustForThisProcess { path: "/repo".into() },
            ClientCommand::TrustState { path: "/repo".into() },
            ClientCommand::CancelRequest { request: RequestId(7) },
        ];
        for command in commands {
            let json = serde_json::to_string(&command).expect("serialize");
            assert_eq!(serde_json::from_str::<ClientCommand>(&json).expect("deserialize"), command);
        }
    }
}
