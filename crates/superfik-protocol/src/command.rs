//! Requests a client sends to a backend.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use superfik_core::project::ProjectRef;
use superfik_core::provider::ModelProvider;
use superfik_core::{
    AskId, BranchRef, ConnectionId, Decision, MemoryEdit, ModelOption, PermissionMode, PluginId, PromptRequest, RunId,
    SessionRecord,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(clippy::large_enum_variant, reason = "commands are short-lived messages, never stored in bulk")]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum ClientCommand {
    Hello {
        version: u32,
    },
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
    InspectWorkspace {
        path: PathBuf,
    },
    ListSkills {
        path: PathBuf,
    },
    SwitchBranch {
        path: PathBuf,
        branch: BranchRef,
    },
    ListAgents {
        path: PathBuf,
    },
    InspectRepository {
        path: PathBuf,
    },
    LoadMemory {
        path: PathBuf,
    },
    SearchMemory {
        path: PathBuf,
        query: String,
    },
    OptimizeMemory {
        run: RunId,
        path: PathBuf,
        model: ModelOption,
    },
    UndoMemory {
        path: PathBuf,
    },
    EditMemory {
        path: PathBuf,
        edits: Vec<MemoryEdit>,
    },
    LoadSessions {
        path: PathBuf,
        #[serde(default)]
        project: Option<ProjectRef>,
    },
    SaveSession {
        path: PathBuf,
        #[serde(default)]
        project: Option<ProjectRef>,
        session: Box<SessionRecord>,
    },
    LoadSettings,
    SaveSettings {
        settings: serde_json::Value,
    },
    ListPlugins,
    InstallPlugin {
        plugin: Option<PluginId>,
        folder: Option<PathBuf>,
    },
    UninstallPlugin {
        plugin: PluginId,
    },
    Connect {
        plugin: PluginId,
        label: String,
        #[serde(default)]
        values: Vec<(String, String)>,
    },
    Disconnect {
        connection: ConnectionId,
    },
    ListProviders,
    ConnectProvider {
        provider: ModelProvider,
        key: String,
    },
    DownloadModel {
        id: String,
    },
    DisconnectProvider {
        provider: ModelProvider,
    },
}

impl ClientCommand {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Hello { .. } => "hello",
            Self::SubmitPrompt { .. } => "submit_prompt",
            Self::CancelRun { .. } => "cancel_run",
            Self::AnswerAsk { .. } => "answer_ask",
            Self::SetPermissionMode { .. } => "set_permission_mode",
            Self::ListModels => "list_models",
            Self::DownloadModel { .. } => "download_model",
            Self::InspectWorkspace { .. } => "inspect_workspace",
            Self::ListSkills { .. } => "list_skills",
            Self::SwitchBranch { .. } => "switch_branch",
            Self::ListAgents { .. } => "list_agents",
            Self::InspectRepository { .. } => "inspect_repository",
            Self::LoadMemory { .. } => "load_memory",
            Self::EditMemory { .. } => "edit_memory",
            Self::SearchMemory { .. } => "search_memory",
            Self::OptimizeMemory { .. } => "optimize_memory",
            Self::UndoMemory { .. } => "undo_memory",
            Self::LoadSessions { .. } => "load_sessions",
            Self::SaveSession { .. } => "save_session",
            Self::ListPlugins => "list_plugins",
            Self::LoadSettings => "load_settings",
            Self::SaveSettings { .. } => "save_settings",
            Self::InstallPlugin { .. } => "install_plugin",
            Self::UninstallPlugin { .. } => "uninstall_plugin",
            Self::Connect { .. } => "connect",
            Self::Disconnect { .. } => "disconnect",
            Self::ListProviders => "list_providers",
            Self::ConnectProvider { .. } => "connect_provider",
            Self::DisconnectProvider { .. } => "disconnect_provider",
        }
    }
}

#[cfg(test)]
mod tests {
    use superfik_core::{Effort, EngineKind, ModelOption, WorkspaceId};

    use super::*;

    fn request() -> PromptRequest {
        PromptRequest::plain(
            PathBuf::from("/repo"),
            "Build a sky",
            ModelOption::new(EngineKind::Native, "claude-opus-5-5", "Opus 5.5"),
        )
    }

    fn session() -> SessionRecord {
        SessionRecord {
            id: superfik_core::ConversationId::new(),
            title: Some("Sky".into()),
            started_unix: 1,
            turns: vec![],
            engine_session: None,
            model: "claude-opus-5-5".into(),
            model_account: None,
            effort: Effort::High,
            archived: true,
            repositories: vec![],
            pull_request: None,
            references: vec![],
            agents: vec![],
            research_items: vec![],
            connections: vec![],
        }
    }

    fn round_trip(command: &ClientCommand) -> ClientCommand {
        let json = serde_json::to_string(command).expect("serialize");
        serde_json::from_str(&json).expect("deserialize")
    }

    #[test]
    fn every_command_round_trips() {
        let run = RunId::new();
        let _ = WorkspaceId::new();
        let commands = [
            ClientCommand::Hello { version: crate::PROTOCOL_VERSION },
            ClientCommand::SubmitPrompt { run, request: request() },
            ClientCommand::CancelRun { run },
            ClientCommand::AnswerAsk {
                run,
                ask: superfik_core::AskId("r1".into()),
                decision: superfik_core::Decision::Answer(vec![vec![0, 2]]),
            },
            ClientCommand::SetPermissionMode { run, mode: PermissionMode::Plan },
            ClientCommand::ListModels,
            ClientCommand::DownloadModel { id: "mlx-community/Qwen3.8-27B-8bit".into() },
            ClientCommand::InspectWorkspace { path: "/repo".into() },
            ClientCommand::ListSkills { path: "/repo".into() },
            ClientCommand::SwitchBranch { path: "/repo".into(), branch: BranchRef::remote("spike") },
            ClientCommand::ListAgents { path: "/repo".into() },
            ClientCommand::InspectRepository { path: "/other".into() },
            ClientCommand::LoadSessions {
                path: "/repo".into(),
                project: Some(ProjectRef::Remote("github.com/a/b".into())),
            },
            ClientCommand::SaveSession { path: "/repo".into(), project: None, session: Box::new(session()) },
            ClientCommand::ListPlugins,
            ClientCommand::LoadSettings,
            ClientCommand::SaveSettings { settings: serde_json::json!({"theme": "dark"}) },
            ClientCommand::InstallPlugin { plugin: None, folder: Some("/p".into()) },
            ClientCommand::UninstallPlugin { plugin: PluginId("linear".into()) },
            ClientCommand::Connect {
                plugin: PluginId("context7".into()),
                label: "me".into(),
                values: vec![("api_key".into(), "k".into())],
            },
            ClientCommand::Disconnect { connection: ConnectionId::new() },
            ClientCommand::ListProviders,
            ClientCommand::ConnectProvider { provider: ModelProvider::Anthropic, key: "sk-ant-x".into() },
            ClientCommand::DisconnectProvider { provider: ModelProvider::Anthropic },
        ];
        for command in &commands {
            assert_eq!(&round_trip(command), command);
        }
    }

    #[test]
    fn commands_are_adjacently_tagged() {
        let json = serde_json::to_value(ClientCommand::ListModels).expect("serialize");
        assert_eq!(json, serde_json::json!({"type": "list_models"}));
        let cancel = serde_json::to_value(ClientCommand::CancelRun { run: RunId::new() }).expect("serialize");
        assert_eq!(cancel["type"], "cancel_run");
        assert!(cancel["data"]["run"].is_string());
        assert_eq!(ClientCommand::CancelRun { run: RunId::new() }.name(), "cancel_run");
    }
}
