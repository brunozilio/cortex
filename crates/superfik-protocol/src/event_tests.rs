use superfik_core::{
    AgentStatus, EngineKind, PullRequest, PullRequestState, Repository, ResearchItem, ResearchKind, SessionId,
    SkillIcon, ToolCall,
};

use super::*;

fn round_trip(event: &ServerEvent) -> ServerEvent {
    let json = serde_json::to_string(event).expect("serialize");
    serde_json::from_str(&json).expect("deserialize")
}

fn run_events() -> Vec<EngineEvent> {
    vec![
        EngineEvent::SessionStarted(SessionId("s".into())),
        EngineEvent::TextDelta("hi".into()),
        EngineEvent::ToolStarted(ToolCall::new("t", "Read", "src")),
        EngineEvent::ToolFinished { id: "t".into() },
        EngineEvent::ResearchFound(ResearchItem {
            title: "Sky".into(),
            url: Some("https://a.dev".into()),
            kind: ResearchKind::Article,
        }),
        EngineEvent::AgentSpawned {
            index: 1,
            kind: "Explore".into(),
            summary: "Map it".into(),
            prompt: "Map how runs resume".into(),
        },
        EngineEvent::AgentEvent {
            index: 1,
            event: Box::new(EngineEvent::ToolStarted(ToolCall::new("s1", "Read", "src/lib.rs"))),
        },
        EngineEvent::AgentUpdate { index: 1, status: AgentStatus::Running { progress: 0.5 }, summary: None },
        EngineEvent::Diagnosed(superfik_core::EngineProblem::NotLoggedIn),
        EngineEvent::Completed { duration_ms: 3, cost_usd: Some(0.25) },
        EngineEvent::Failed("x".into()),
        EngineEvent::Cancelled,
    ]
}

#[test]
fn every_event_round_trips() {
    let run = RunId::new();
    let mut events: Vec<ServerEvent> = run_events().into_iter().map(|event| ServerEvent::Run { run, event }).collect();
    events.push(ServerEvent::Welcome { version: crate::PROTOCOL_VERSION, principal: Principal::LocalOwner });
    events.push(ServerEvent::Members(vec![Colleague {
        user: superfik_core::UserId::new(),
        name: "ana".into(),
        role: superfik_core::access::OrgRole::Admin,
        pending: true,
    }]));
    events.push(ServerEvent::FirstAccessCode {
        user: superfik_core::UserId::new(),
        name: "ana".into(),
        code: "ABCD-EFGH-JKMN".into(),
    });
    events
        .push(ServerEvent::ServerSettings(crate::carried::ServerSettings { domain: Some("superfik.acme.io".into()) }));
    events.push(ServerEvent::Models(vec![ModelOption::new(EngineKind::Native, "opus", "Opus")]));
    events.push(ServerEvent::Workspace(WorkspaceSnapshot {
        repository: Some(Repository {
            root: PathBuf::from("/repo"),
            slug: "a/b".into(),
            project: Some(superfik_core::project::ProjectRef::Remote("github.com/a/b".into())),
            head_short_sha: "abc".into(),
            head_subject: "Init".into(),
            branch: "main".into(),
            branches: vec!["main".into()],
            remote_branches: vec!["spike".into()],
            pull_request: Some(PullRequest {
                number: 4,
                title: "Sky".into(),
                state: PullRequestState::Draft,
                ..PullRequest::default()
            }),
        }),
    }));
    events.push(ServerEvent::Repository(Repository {
        root: PathBuf::from("/other"),
        slug: "acme/other".into(),
        ..Repository::default()
    }));
    events.push(ServerEvent::GitHubRepositories {
        signed_in: true,
        repositories: vec![GitHubRepository { slug: "acme/rocket".into(), private: false }],
    });
    events.push(ServerEvent::RepositoryLocated { slug: "acme/rocket".into(), path: PathBuf::from("/w/rocket") });
    events.push(ServerEvent::PullRequests(vec![PullRequest {
        number: 4,
        title: "Sky".into(),
        ..PullRequest::default()
    }]));
    events.push(ServerEvent::PullRequestCreated(PullRequest { number: 5, ..PullRequest::default() }));
    events.push(ServerEvent::PullRequestProposed(PullRequestDraft {
        title: "Sky".into(),
        ..PullRequestDraft::default()
    }));
    events.push(ServerEvent::ReviewSubmitted { number: 5 });
    events.push(ServerEvent::ReviewDrafted { number: 5, summary: "ok".into(), comments: vec![] });
    events.push(ServerEvent::ThreadResolved { thread: "T1".into(), resolved: true });
    events.push(ServerEvent::PullRequestLoaded(Box::new(PullRequestDetail {
        pull_request: PullRequest { number: 5, ..PullRequest::default() },
        description: String::new(),
        files: vec![],
        comments: vec![],
    })));
    events.push(ServerEvent::Skills(vec![Skill {
        name: "m".into(),
        title: "M skill".into(),
        description: String::new(),
        icon: SkillIcon::Book,
    }]));
    events.push(ServerEvent::Engines(vec![EngineInfo {
        kind: EngineKind::Codex,
        installed: true,
        implemented: false,
        models: vec![],
    }]));
    events.push(ServerEvent::Agents(vec![CustomAgent { name: "reviewer".into(), description: "R".into() }]));
    events.push(ServerEvent::Sessions { path: "/repo".into(), sessions: vec![] });
    let project = superfik_core::project::ProjectRef::Remote("github.com/a/b".into());
    events.push(ServerEvent::SharedSessions { project, sessions: vec![] });
    events.push(ServerEvent::Delegated(vec![]));
    events.push(ServerEvent::Library(vec![]));
    events.push(ServerEvent::Comments {
        session: superfik_core::ConversationId::new(),
        comments: vec![superfik_core::comments::Comment {
            author: superfik_core::UserId::new(),
            author_name: "Ana".into(),
            at_unix: 1,
            after_turn: 0,
            text: "Looks good".into(),
        }],
    });
    events.push(ServerEvent::Teams(vec![superfik_core::sharing::Team {
        id: superfik_core::TeamId::new(),
        name: "Design".into(),
        members: vec![superfik_core::UserId::new()],
    }]));
    events.push(ServerEvent::Policy(Default::default()));
    events.push(ServerEvent::Spent { month: "2026-10".into(), usd: 12.5 });
    events.push(ServerEvent::Usage(vec![MemberUsage {
        user: superfik_core::UserId::new(),
        name: "Ana".into(),
        month: "2026-10".into(),
        cost_usd: 12.5,
        runs: 3,
    }]));
    let viewer = Viewer {
        user: superfik_core::UserId::new(),
        name: "Ana".into(),
        session: superfik_core::ConversationId::new(),
    };
    events.push(ServerEvent::Presence(vec![viewer]));
    let project = superfik_core::project::ProjectRef::Remote("github.com/a/b".into());
    events.push(ServerEvent::SharedMemory { project, memories: vec![] });
    events.push(ServerEvent::Notes {
        path: "/repo".into(),
        notes: vec![],
        by: Some(RunId::new()),
        created: vec![superfik_core::note::NoteId(1)],
        refused: vec![],
    });
    events.push(ServerEvent::DelegatedAnswered {
        run: RunId::new(),
        ask: superfik_core::AskId("p1".into()),
        decision: superfik_core::Decision::Deny,
        by: "Bia".into(),
    });
    events.push(ServerEvent::Plugins { plugins: vec![], connections: vec![] });
    events.push(ServerEvent::AuthorizeAt { plugin: PluginId("linear".into()), url: "https://l".into() });
    events.push(ServerEvent::PluginFailed { plugin: None, message: "bad".into() });
    {
        use superfik_core::provider::{ModelProvider, ProviderAccount, ProviderSummary};
        let account = ProviderAccount { label: "…ab12".into(), since_unix: 1 };
        let providers = vec![ProviderSummary { provider: ModelProvider::Anthropic, account: Some(account) }];
        let provider = ModelProvider::OpenAi;
        events.push(ServerEvent::Providers(crate::ProviderEvent::Listed { providers }));
        for state in [
            crate::DownloadState::Started,
            crate::DownloadState::Preparing,
            crate::DownloadState::Fetching { done: 600, total: Some(1000) },
            crate::DownloadState::Fetching { done: 600, total: None },
            crate::DownloadState::Done,
            crate::DownloadState::Failed("disk".into()),
        ] {
            events.push(ServerEvent::Providers(crate::ProviderEvent::Download { id: "org/qwen".into(), state }));
        }
        events.push(ServerEvent::Providers(crate::ProviderEvent::Failed { provider, message: "no".into() }));
    }
    let browser = BrowserId::new();
    let url = String::from("https://a.com/login");
    let candidates = vec![Candidate { item: "A".into(), account: "Bitwarden".into(), username: Some("b".into()) }];
    events.push(ServerEvent::LoginCandidates { browser, url: url.clone(), candidates });
    let fill = LoginFill { username: None, password: Some("p".into()) };
    events.push(ServerEvent::LoginSecret { browser, url, fill });
    events.push(ServerEvent::LoginFailed { browser, message: "locked".into() });
    events.push(ServerEvent::LoginSaved { browser, account: "Bitwarden".into() });
    events.push(ServerEvent::error("nope"));
    events.push(ServerEvent::Link(crate::LinkState::Lost));
    events.push(ServerEvent::GitHubCode { code: "ABCD-1234".into(), url: "https://github.com/login/device".into() });
    events.push(ServerEvent::GitHubSignedIn { login: Some("olga".into()) });
    events.push(ServerEvent::Settings { settings: Some(serde_json::json!({"theme": "dark"})) });
    events.push(ServerEvent::Settings { settings: None });
    events.push(ServerEvent::Uploaded { from: "/Users/olga/sky.png".into(), path: "/srv/u/uploads/sky.png".into() });
    for event in &events {
        assert_eq!(&round_trip(event), event);
    }
}

#[test]
fn terminal_detection() {
    let run = RunId::new();
    assert!(ServerEvent::Run { run, event: EngineEvent::Cancelled }.is_run_terminal());
    assert!(!ServerEvent::Run { run, event: EngineEvent::TextDelta("x".into()) }.is_run_terminal());
    assert!(!ServerEvent::error("x").is_run_terminal());
}
