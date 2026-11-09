//! Requests a client sends to a backend.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use superfik_core::project::ProjectRef;
use superfik_core::provider::ModelProvider;
use superfik_core::{
    AskId, BranchRef, BrowserId, ConnectionId, Decision, MemoryEdit, ModelOption, PermissionMode, PluginId,
    PromptRequest, PullRequestDraft, ReviewCommentDraft, ReviewVerdict, RunId, SessionRecord, UserId,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(clippy::large_enum_variant, reason = "commands are short-lived messages, never stored in bulk")]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum ClientCommand {
    Hello {
        version: u32,
    },
    ListMembers,
    AddUser {
        name: String,
        role: superfik_core::access::OrgRole,
    },
    NewFirstAccessCode {
        user: UserId,
    },
    RemoveUser {
        user: UserId,
    },
    SetUserRole {
        user: UserId,
        role: superfik_core::access::OrgRole,
    },
    LoadServerSettings,
    SaveServerSettings {
        domain: String,
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
    DelegateAsk {
        run: RunId,
        ask: superfik_core::AgentAsk,
        session: Option<String>,
    },
    ListDelegated,
    AnswerDelegated {
        run: RunId,
        ask: AskId,
        decision: Decision,
    },
    LoadSharedMemory {
        project: ProjectRef,
    },
    ShareMemory {
        project: ProjectRef,
        scope: superfik_core::sharing::MemoryScope,
        memory: superfik_core::Memory,
    },
    UnshareMemory {
        project: ProjectRef,
        scope: superfik_core::sharing::MemoryScope,
        id: superfik_core::MemoryId,
    },
    Viewing {
        session: Option<superfik_core::ConversationId>,
    },
    GetPolicy,
    SetPolicy {
        policy: superfik_core::policy::Policy,
    },
    ReportUsage {
        run: RunId,
        model: String,
        cost_usd: f64,
    },
    ListUsage,
    LoadComments {
        project: ProjectRef,
        session: superfik_core::ConversationId,
    },
    AddComment {
        project: ProjectRef,
        session: superfik_core::ConversationId,
        after_turn: usize,
        text: String,
    },
    ListTeams,
    CreateTeam {
        name: String,
    },
    RemoveTeam {
        team: superfik_core::TeamId,
    },
    SetTeamMember {
        team: superfik_core::TeamId,
        user: UserId,
        member: bool,
    },
    ListLibrary,
    PublishToLibrary {
        item: superfik_core::library::LibraryItem,
    },
    RemoveFromLibrary {
        kind: superfik_core::library::LibraryKind,
        name: String,
    },
    InstallLibrary {
        items: Vec<superfik_core::library::LibraryItem>,
    },
    ReadLibraryItem {
        path: PathBuf,
        name: String,
    },
    AnswerBrowse {
        run: RunId,
        id: superfik_core::BrowseId,
        answer: Result<superfik_core::BrowserAnswer, String>,
    },
    Page(superfik_core::page::PageCall),
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
    ListGitHubRepositories,
    LocateRepository {
        slug: String,
        known: Vec<PathBuf>,
        clone_root: PathBuf,
    },
    ListPullRequests {
        path: PathBuf,
    },
    CheckoutPullRequest {
        path: PathBuf,
        number: u64,
    },
    CreatePullRequest {
        path: PathBuf,
        draft: PullRequestDraft,
    },
    LoadPullRequest {
        path: PathBuf,
        number: u64,
    },
    SubmitReview {
        path: PathBuf,
        number: u64,
        verdict: ReviewVerdict,
        body: String,
        comments: Vec<ReviewCommentDraft>,
    },
    ResolveThread {
        path: PathBuf,
        thread: String,
        resolved: bool,
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
    LoadNotes {
        path: PathBuf,
    },
    EditNotes {
        path: PathBuf,
        edits: Vec<superfik_core::note::NoteEdit>,
    },
    LoadRoutines {
        path: PathBuf,
    },
    EditRoutines {
        path: PathBuf,
        edits: Vec<superfik_core::routine::RoutineEdit>,
    },
    RevealWebhook {
        path: PathBuf,
        routine: superfik_core::routine::RoutineId,
    },
    ListChannels,
    ApproveDraft {
        connection: superfik_core::ConnectionId,
        draft: u32,
        text: String,
    },
    DiscardDraft {
        connection: superfik_core::ConnectionId,
        draft: u32,
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
    Upload {
        from: PathBuf,
        name: String,
        data: String,
    },
    LoadSettings,
    SaveSettings {
        settings: serde_json::Value,
    },
    SignInToGitHub,
    SignOutOfGitHub,
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
    FindLogins {
        browser: BrowserId,
        url: String,
    },
    FillLogin {
        browser: BrowserId,
        url: String,
        account: String,
        item: String,
        #[serde(default)]
        run: Option<RunId>,
    },
    SaveLogin {
        browser: BrowserId,
        account: String,
        login: superfik_core::autofill::NewLogin,
    },
    FindPasskeys {
        browser: BrowserId,
        rp_id: String,
        #[serde(default)]
        allowed: Vec<String>,
    },
    SignPasskey {
        browser: BrowserId,
        chosen: superfik_core::passkey::PasskeyUse,
    },
    CreatePasskey {
        browser: BrowserId,
        new: superfik_core::passkey::NewPasskey,
    },
}

impl ClientCommand {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Hello { .. } => "hello",
            Self::ListMembers => "list_members",
            Self::AddUser { .. } => "add_user",
            Self::NewFirstAccessCode { .. } => "new_first_access_code",
            Self::RemoveUser { .. } => "remove_user",
            Self::SetUserRole { .. } => "set_user_role",
            Self::LoadServerSettings => "load_server_settings",
            Self::SaveServerSettings { .. } => "save_server_settings",
            Self::SubmitPrompt { .. } => "submit_prompt",
            Self::CancelRun { .. } => "cancel_run",
            Self::AnswerAsk { .. } => "answer_ask",
            Self::DelegateAsk { .. } => "delegate_ask",
            Self::ListDelegated => "list_delegated",
            Self::AnswerDelegated { .. } => "answer_delegated",
            Self::LoadSharedMemory { .. } => "load_shared_memory",
            Self::ShareMemory { .. } => "share_memory",
            Self::UnshareMemory { .. } => "unshare_memory",
            Self::Viewing { .. } => "viewing",
            Self::GetPolicy => "get_policy",
            Self::SetPolicy { .. } => "set_policy",
            Self::ReportUsage { .. } => "report_usage",
            Self::ListUsage => "list_usage",
            Self::LoadComments { .. } => "load_comments",
            Self::AddComment { .. } => "add_comment",
            Self::ListTeams => "list_teams",
            Self::CreateTeam { .. } => "create_team",
            Self::RemoveTeam { .. } => "remove_team",
            Self::SetTeamMember { .. } => "set_team_member",
            Self::ListLibrary => "list_library",
            Self::PublishToLibrary { .. } => "publish_to_library",
            Self::RemoveFromLibrary { .. } => "remove_from_library",
            Self::InstallLibrary { .. } => "install_library",
            Self::ReadLibraryItem { .. } => "read_library_item",
            Self::AnswerBrowse { .. } => "answer_browse",
            Self::Page(_) => "page",
            Self::SetPermissionMode { .. } => "set_permission_mode",
            Self::ListModels => "list_models",
            Self::DownloadModel { .. } => "download_model",
            Self::InspectWorkspace { .. } => "inspect_workspace",
            Self::ListSkills { .. } => "list_skills",
            Self::SwitchBranch { .. } => "switch_branch",
            Self::ListAgents { .. } => "list_agents",
            Self::InspectRepository { .. } => "inspect_repository",
            Self::ListGitHubRepositories => "list_github_repositories",
            Self::LocateRepository { .. } => "locate_repository",
            Self::ListPullRequests { .. } => "list_pull_requests",
            Self::CheckoutPullRequest { .. } => "checkout_pull_request",
            Self::CreatePullRequest { .. } => "create_pull_request",
            Self::LoadPullRequest { .. } => "load_pull_request",
            Self::SubmitReview { .. } => "submit_review",
            Self::ResolveThread { .. } => "resolve_thread",
            Self::LoadMemory { .. } => "load_memory",
            Self::EditMemory { .. } => "edit_memory",
            Self::SearchMemory { .. } => "search_memory",
            Self::OptimizeMemory { .. } => "optimize_memory",
            Self::UndoMemory { .. } => "undo_memory",
            Self::LoadNotes { .. } => "load_notes",
            Self::EditNotes { .. } => "edit_notes",
            Self::LoadRoutines { .. } => "load_routines",
            Self::EditRoutines { .. } => "edit_routines",
            Self::RevealWebhook { .. } => "reveal_webhook",
            Self::ListChannels => "list_channels",
            Self::ApproveDraft { .. } => "approve_draft",
            Self::DiscardDraft { .. } => "discard_draft",
            Self::LoadSessions { .. } => "load_sessions",
            Self::SaveSession { .. } => "save_session",
            Self::ListPlugins => "list_plugins",
            Self::Upload { .. } => "upload",
            Self::LoadSettings => "load_settings",
            Self::SaveSettings { .. } => "save_settings",
            Self::SignInToGitHub => "sign_in_to_github",
            Self::SignOutOfGitHub => "sign_out_of_github",
            Self::InstallPlugin { .. } => "install_plugin",
            Self::UninstallPlugin { .. } => "uninstall_plugin",
            Self::Connect { .. } => "connect",
            Self::Disconnect { .. } => "disconnect",
            Self::ListProviders => "list_providers",
            Self::ConnectProvider { .. } => "connect_provider",
            Self::DisconnectProvider { .. } => "disconnect_provider",
            Self::FindLogins { .. } => "find_logins",
            Self::FillLogin { .. } => "fill_login",
            Self::SaveLogin { .. } => "save_login",
            Self::FindPasskeys { .. } => "find_passkeys",
            Self::SignPasskey { .. } => "sign_passkey",
            Self::CreatePasskey { .. } => "create_passkey",
        }
    }
}

#[cfg(test)]
mod tests {
    use superfik_core::{Effort, EngineKind, ModelOption, WorkspaceId};

    use super::*;

    fn request() -> PromptRequest {
        PromptRequest {
            workspace: WorkspaceId::new(),
            cwd: PathBuf::from("/repo"),
            prompt: "Build a sky".into(),
            model: ModelOption::new(EngineKind::Native, "claude-opus-5-5", "Opus 5.5"),
            effort: Effort::High,
            repositories: vec![],
            skills: vec![],
            disabled_skills: Vec::new(),
            references: vec![],
            notes: vec![],
            resume: None,
            command: None,
            handoff: None,
            memory: None,
            permission: Default::default(),
            output_style: None,
            safety_check: true,
            connections: vec![],
            plugins: None,
            browser: None,
            credentials: Default::default(),
        }
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
            notes: vec![],
            visibility: Default::default(),
        }
    }

    fn round_trip(command: &ClientCommand) -> ClientCommand {
        let json = serde_json::to_string(command).expect("serialize");
        serde_json::from_str(&json).expect("deserialize")
    }

    #[test]
    fn every_command_round_trips() {
        let run = RunId::new();
        let commands = [
            ClientCommand::Hello { version: crate::PROTOCOL_VERSION },
            ClientCommand::ListMembers,
            ClientCommand::AddUser { name: "ana".into(), role: superfik_core::access::OrgRole::Member },
            ClientCommand::NewFirstAccessCode { user: UserId::new() },
            ClientCommand::RemoveUser { user: UserId::new() },
            ClientCommand::SetUserRole { user: UserId::new(), role: superfik_core::access::OrgRole::Admin },
            ClientCommand::LoadServerSettings,
            ClientCommand::SaveServerSettings { domain: "superfik.acme.io".into() },
            ClientCommand::SubmitPrompt { run, request: request() },
            ClientCommand::CancelRun { run },
            ClientCommand::AnswerAsk {
                run,
                ask: superfik_core::AskId("r1".into()),
                decision: superfik_core::Decision::Answer(vec![vec![0, 2]]),
            },
            ClientCommand::DelegateAsk {
                run,
                ask: superfik_core::AgentAsk {
                    id: superfik_core::AskId("p1".into()),
                    kind: superfik_core::AskKind::Permission {
                        tool: "Bash".into(),
                        subject: "cargo publish".into(),
                        rememberable: false,
                    },
                },
                session: Some("Release".into()),
            },
            ClientCommand::ListDelegated,
            ClientCommand::ListLibrary,
            ClientCommand::ListTeams,
            ClientCommand::LoadComments {
                project: ProjectRef::Remote("github.com/a/b".into()),
                session: superfik_core::ConversationId::new(),
            },
            ClientCommand::AddComment {
                project: ProjectRef::Remote("github.com/a/b".into()),
                session: superfik_core::ConversationId::new(),
                after_turn: 2,
                text: "Looks good".into(),
            },
            ClientCommand::CreateTeam { name: "Design".into() },
            ClientCommand::RemoveTeam { team: superfik_core::TeamId::new() },
            ClientCommand::SetTeamMember { team: superfik_core::TeamId::new(), user: UserId::new(), member: true },
            ClientCommand::Viewing { session: Some(superfik_core::ConversationId::new()) },
            ClientCommand::GetPolicy,
            ClientCommand::ListUsage,
            ClientCommand::SetPolicy {
                policy: superfik_core::policy::Policy {
                    models: Some(vec!["claude-sonnet-5-5".into()]),
                    max_mode: Some(PermissionMode::AcceptEdits),
                    monthly_budget_usd: Some(50.0),
                },
            },
            ClientCommand::ReportUsage { run, model: "claude-opus-5-5".into(), cost_usd: 0.25 },
            ClientCommand::RemoveFromLibrary { kind: superfik_core::library::LibraryKind::Agent, name: "r".into() },
            ClientCommand::ReadLibraryItem { path: "/repo".into(), name: "motion".into() },
            ClientCommand::InstallLibrary {
                items: vec![superfik_core::library::LibraryItem {
                    kind: superfik_core::library::LibraryKind::Skill,
                    name: "motion".into(),
                    body: "---\nname: motion\n---".into(),
                    author_name: "Olga".into(),
                }],
            },
            ClientCommand::LoadSharedMemory { project: ProjectRef::Remote("github.com/a/b".into()) },
            ClientCommand::UnshareMemory {
                project: ProjectRef::Remote("github.com/a/b".into()),
                scope: superfik_core::sharing::MemoryScope::Organization,
                id: superfik_core::MemoryId(3),
            },
            ClientCommand::AnswerDelegated {
                run,
                ask: superfik_core::AskId("p1".into()),
                decision: superfik_core::Decision::Allow,
            },
            ClientCommand::AnswerBrowse {
                run,
                id: superfik_core::BrowseId("b1".into()),
                answer: Ok(superfik_core::BrowserAnswer::text("Example Domain")),
            },
            ClientCommand::SetPermissionMode { run, mode: PermissionMode::Plan },
            ClientCommand::ListModels,
            ClientCommand::DownloadModel { id: "mlx-community/Qwen3.8-27B-8bit".into() },
            ClientCommand::InspectWorkspace { path: "/repo".into() },
            ClientCommand::ListSkills { path: "/repo".into() },
            ClientCommand::SwitchBranch { path: "/repo".into(), branch: BranchRef::remote("spike") },
            ClientCommand::ListAgents { path: "/repo".into() },
            ClientCommand::InspectRepository { path: "/other".into() },
            ClientCommand::ListGitHubRepositories,
            ClientCommand::ListPullRequests { path: "/repo".into() },
            ClientCommand::CheckoutPullRequest { path: "/repo".into(), number: 42 },
            ClientCommand::CreatePullRequest {
                path: "/repo".into(),
                draft: PullRequestDraft { title: "Sky".into(), ..PullRequestDraft::default() },
            },
            ClientCommand::LoadPullRequest { path: "/repo".into(), number: 5 },
            ClientCommand::SubmitReview {
                path: "/repo".into(),
                number: 5,
                verdict: ReviewVerdict::Approve,
                body: String::new(),
                comments: vec![],
            },
            ClientCommand::ResolveThread { path: "/repo".into(), thread: "T1".into(), resolved: true },
            ClientCommand::LocateRepository {
                slug: "acme/rocket".into(),
                known: vec!["/repo".into()],
                clone_root: "/w".into(),
            },
            ClientCommand::LoadSessions {
                path: "/repo".into(),
                project: Some(ProjectRef::Remote("github.com/a/b".into())),
            },
            ClientCommand::SaveSession { path: "/repo".into(), project: None, session: Box::new(session()) },
            ClientCommand::ListPlugins,
            ClientCommand::SignInToGitHub,
            ClientCommand::SignOutOfGitHub,
            ClientCommand::LoadSettings,
            ClientCommand::SaveSettings { settings: serde_json::json!({"theme": "dark", "onboarded": true}) },
            ClientCommand::Upload { from: "/Users/olga/sky.png".into(), name: "sky.png".into(), data: "AAEC".into() },
            ClientCommand::InstallPlugin { plugin: Some(PluginId("linear".into())), folder: None },
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
            ClientCommand::FindLogins { browser: BrowserId::new(), url: "https://a.com/login".into() },
            ClientCommand::FillLogin {
                browser: BrowserId::new(),
                url: "https://a.com/login".into(),
                account: "Bitwarden · Superfik".into(),
                item: "A".into(),
                run: Some(run),
            },
            ClientCommand::SaveLogin {
                browser: BrowserId::new(),
                account: "Bitwarden · Superfik".into(),
                login: superfik_core::autofill::NewLogin {
                    name: "a.com".into(),
                    url: "https://a.com/login".into(),
                    username: "bruno".into(),
                    password: "p".into(),
                },
            },
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
