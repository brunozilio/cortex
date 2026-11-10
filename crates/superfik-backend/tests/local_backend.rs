//! LocalBackend end to end, with a scripted engine and fake workspace IO.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_channel::Receiver;
use superfik_backend::{Backend, InMemoryStore, LocalBackend, LocalServices, Memories, Wake};
use superfik_core::{
    AgentAsk, AskId, AskKind, CustomAgent, Decision, Effort, EngineEvent, EngineKind, ModelOption, Principal,
    PromptRequest, Repository, RunId, Skill, SkillIcon, UserId, WorkspaceId,
};
use superfik_engine::fake::FakeEngine;
use superfik_engine::{EngineRegistry, PlannedEngine};
use superfik_plugins::{AgentCatalog, SkillCatalog};
use superfik_protocol::{ClientCommand, Envelope, ServerEvent};
use superfik_workspace::{GitInspector, WorkspaceError};

struct FakeGit;
impl GitInspector for FakeGit {
    fn inspect(&self, root: &Path) -> Result<Repository, WorkspaceError> {
        Ok(Repository {
            root: root.to_path_buf(),
            slug: "acme/rocket".into(),
            head_short_sha: "abc1234".into(),
            head_subject: "Launch".into(),
            branch: "main".into(),
            branches: vec!["main".into()],
            ..Repository::default()
        })
    }
}

struct FakeSkills;
impl SkillCatalog for FakeSkills {
    fn skills(&self, _repo: &Path) -> Vec<Skill> {
        vec![Skill {
            name: "motion".into(),
            title: "Motion skill".into(),
            description: String::new(),
            icon: SkillIcon::Bolt,
        }]
    }
}

struct FakeAgents;
impl AgentCatalog for FakeAgents {
    fn agents(&self, _repo: &Path) -> Vec<CustomAgent> {
        vec![CustomAgent { name: "reviewer".into(), description: "Reviews".into() }]
    }
}

fn model() -> ModelOption {
    ModelOption::new(EngineKind::Native, "claude-opus-5-5", "Opus 5.5")
}

fn backend(engine: FakeEngine) -> (LocalBackend, Receiver<()>) {
    let planned = vec![PlannedEngine { kind: EngineKind::Codex, installed: false }];
    let mut services = LocalServices::with_engines(EngineRegistry::new().with(Arc::new(engine)).with_planned(planned));
    services.git = Arc::new(FakeGit);
    services.skills = Arc::new(FakeSkills);
    services.agents = Arc::new(FakeAgents);
    services.memory = Memories::plain(Arc::new(InMemoryStore::default()));
    services.memory_tool = false;
    services.plugins = superfik_backend::Plugins::new(
        Arc::new(superfik_backend::InMemoryPluginStore::new(tempfile::tempdir().expect("packages").keep())),
        Arc::new(superfik_backend::InMemorySecretStore::default()),
        Arc::new(superfik_backend::McpOAuth::default()),
        Arc::new(superfik_backend::SystemTokenCommand),
    );
    services.providers = services.providers.with_store(Arc::new(superfik_backend::InMemoryProviderStore::default()));
    let (woke_tx, woke) = crossbeam_channel::unbounded();
    let wake: Wake = Arc::new(move || {
        let _ = woke_tx.send(());
    });
    (LocalBackend::new(services, wake).expect("backend"), woke)
}

fn scripted() -> FakeEngine {
    FakeEngine::new(
        EngineKind::Native,
        vec![model()],
        vec![EngineEvent::TextDelta("Hello".into()), EngineEvent::Completed { duration_ms: 10, cost_usd: None }],
    )
}

fn request(engine: EngineKind) -> PromptRequest {
    PromptRequest {
        connections: Vec::new(),
        plugins: None,
        credentials: Default::default(),
        workspace: WorkspaceId::new(),
        cwd: PathBuf::from("/repo"),
        prompt: "hi".into(),
        model: ModelOption { engine, ..model() },
        effort: Effort::Medium,
        repositories: vec![],
        skills: vec![],
        disabled_skills: Vec::new(),
        references: vec![],
        resume: None,
        command: None,
        memory: None,
        permission: Default::default(),
        output_style: None,
        safety_check: true,
    }
}

fn collect(backend: &LocalBackend, woke: &Receiver<()>, done: impl Fn(&ServerEvent) -> bool) -> Vec<ServerEvent> {
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut events = Vec::new();
    while Instant::now() < deadline {
        let _ = woke.recv_timeout(Duration::from_millis(50));
        while let Some(event) = backend.try_recv() {
            let finished = done(&event);
            events.push(event);
            if finished {
                return events;
            }
        }
    }
    panic!("timed out; got {events:?}");
}

fn send(backend: &LocalBackend, command: ClientCommand) {
    backend.send(Envelope::local(command)).expect("send");
}

#[test]
fn submit_streams_run_events_and_wakes() {
    let (backend, woke) = backend(scripted());
    let run = RunId::new();
    send(&backend, ClientCommand::SubmitPrompt { run, request: request(EngineKind::Native) });
    let events = collect(&backend, &woke, ServerEvent::is_run_terminal);
    assert_eq!(
        events,
        vec![
            ServerEvent::Run { run, event: EngineEvent::TextDelta("Hello".into()) },
            ServerEvent::Run { run, event: EngineEvent::Completed { duration_ms: 10, cost_usd: None } },
        ]
    );
}

#[test]
fn cancel_stops_a_slow_run() {
    let (backend, woke) = backend(FakeEngine::demo().with_delay(Duration::from_millis(200)));
    let run = RunId::new();
    send(&backend, ClientCommand::SubmitPrompt { run, request: request(EngineKind::Native) });
    send(&backend, ClientCommand::CancelRun { run });
    let events = collect(&backend, &woke, ServerEvent::is_run_terminal);
    assert_eq!(events.last(), Some(&ServerEvent::Run { run, event: EngineEvent::Cancelled }));
}

#[test]
fn answers_reach_the_run_that_asked() {
    let ask = AgentAsk {
        id: AskId("r1".into()),
        kind: AskKind::Permission { tool: "Bash".into(), subject: "ls".into(), rememberable: false },
    };
    let engine = FakeEngine::new(EngineKind::Native, vec![model()], vec![EngineEvent::Asked(ask.clone())]);
    let (backend, woke) = backend(engine.clone());
    let run = RunId::new();
    send(&backend, ClientCommand::SubmitPrompt { run, request: request(EngineKind::Native) });
    collect(&backend, &woke, |event| matches!(event, ServerEvent::Run { event: EngineEvent::Asked(_), .. }));
    send(&backend, ClientCommand::AnswerAsk { run, ask: ask.id.clone(), decision: Decision::Deny });
    send(&backend, ClientCommand::AnswerAsk { run: RunId::new(), ask: ask.id.clone(), decision: Decision::Allow });
    let deadline = Instant::now() + Duration::from_secs(30);
    while engine.answers().is_empty() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(2));
    }
    std::thread::sleep(Duration::from_millis(20));
    assert_eq!(engine.answers(), vec![(ask.id, Decision::Deny)]);
}

#[test]
fn unknown_engine_fails_the_run() {
    let (backend, woke) = backend(scripted());
    let run = RunId::new();
    send(&backend, ClientCommand::SubmitPrompt { run, request: request(EngineKind::Codex) });
    let events = collect(&backend, &woke, ServerEvent::is_run_terminal);
    assert_eq!(
        events,
        vec![ServerEvent::Run { run, event: EngineEvent::Failed("no engine registered for Codex".into()) }]
    );
}

#[test]
fn queries_answer_models_workspace_and_skills() {
    let (backend, woke) = backend(scripted());
    send(&backend, ClientCommand::ListModels);
    let models = collect(&backend, &woke, |event| matches!(event, ServerEvent::Models(_)));
    assert_eq!(models, vec![ServerEvent::Models(vec![model()])]);
    match &collect(&backend, &woke, |event| matches!(event, ServerEvent::Engines(_)))[..] {
        [ServerEvent::Engines(infos)] => {
            assert_eq!(infos.iter().map(|i| i.kind).collect::<Vec<_>>(), vec![EngineKind::Native, EngineKind::Codex]);
            assert!(infos[0].is_selectable() && !infos[1].is_selectable());
        }
        other => panic!("unexpected {other:?}"),
    }

    let dir = tempfile::tempdir().expect("tempdir");
    send(&backend, ClientCommand::InspectWorkspace { path: dir.path().to_path_buf() });
    match &collect(&backend, &woke, |event| matches!(event, ServerEvent::Workspace(_)))[..] {
        [ServerEvent::Workspace(snapshot)] => {
            assert_eq!(snapshot.repository.as_ref().map(|repo| repo.slug.as_str()), Some("acme/rocket"));
        }
        other => panic!("unexpected {other:?}"),
    }

    send(&backend, ClientCommand::ListSkills { path: "/repo".into() });
    let skills = collect(&backend, &woke, |event| matches!(event, ServerEvent::Skills(_)));
    assert!(matches!(&skills[..], [ServerEvent::Skills(list)] if list[0].name == "motion"));

    send(&backend, ClientCommand::ListAgents { path: "/repo".into() });
    let agents = collect(&backend, &woke, |event| matches!(event, ServerEvent::Agents(_)));
    assert!(matches!(&agents[..], [ServerEvent::Agents(list)] if list[0].name == "reviewer"));

    send(&backend, ClientCommand::InspectWorkspace { path: dir.path().join("missing") });
    let error = collect(&backend, &woke, |event| matches!(event, ServerEvent::Error { .. }));
    assert_eq!(error.len(), 1);
}

#[test]
fn a_client_speaking_this_protocol_is_welcomed() {
    let (backend, woke) = backend(scripted());
    send(&backend, ClientCommand::Hello { version: superfik_protocol::PROTOCOL_VERSION });
    let welcome = collect(&backend, &woke, |event| matches!(event, ServerEvent::Welcome { .. }));
    assert_eq!(
        welcome,
        [ServerEvent::Welcome { version: superfik_protocol::PROTOCOL_VERSION, principal: Principal::LocalOwner }]
    );
}

#[test]
fn a_client_speaking_another_protocol_is_told_both_versions() {
    let (backend, woke) = backend(scripted());
    send(&backend, ClientCommand::Hello { version: 999 });
    match &collect(&backend, &woke, |event| matches!(event, ServerEvent::Error { .. }))[..] {
        [ServerEvent::Error { message }] => {
            assert!(message.contains("999"), "{message}");
            assert!(message.contains(&superfik_protocol::PROTOCOL_VERSION.to_string()), "{message}");
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn remote_principals_are_rejected_locally() {
    let (backend, woke) = backend(scripted());
    backend
        .send(Envelope::new(Principal::member(UserId::new(), superfik_core::OrgId::new()), ClientCommand::ListModels))
        .expect("send");
    match &collect(&backend, &woke, |event| matches!(event, ServerEvent::Error { .. }))[..] {
        [ServerEvent::Error { message }] => assert!(message.contains("may not list_models")),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn dropping_the_backend_cancels_runs() {
    let (backend, _woke) = backend(FakeEngine::demo().with_delay(Duration::from_secs(5)));
    send(&backend, ClientCommand::SubmitPrompt { run: RunId::new(), request: request(EngineKind::Native) });
    let started = Instant::now();
    drop(backend);
    assert!(started.elapsed() < Duration::from_secs(2));
}
