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
    events.push(ServerEvent::error("nope"));
    events.push(ServerEvent::Settings { settings: Some(serde_json::json!({"theme": "dark"})) });
    events.push(ServerEvent::Settings { settings: None });
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
