//! LocalBackend end to end, with a scripted engine.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use agent_engine::fake::FakeEngine;
use backend::{Backend, InMemoryStore, LocalBackend, LocalServices, Memories, Wake};
use crossbeam_channel::Receiver;
use domain::{AgentAsk, AskId, AskKind, Decision, EngineEvent, ModelOption, Principal, PromptRequest, RunId, UserId};
use protocol::{ClientCommand, Envelope, ServerEvent};

/// A host folder with one skill and one agent, as the SDK would add.
fn sdk_folder() -> PathBuf {
    let root = tempfile::tempdir().expect("sdk").keep();
    std::fs::create_dir_all(root.join("skills/motion")).expect("skills");
    std::fs::create_dir_all(root.join("agents")).expect("agents");
    std::fs::write(root.join("skills/motion/SKILL.md"), "---\nname: motion\ndescription: Motion\n---\nMove.")
        .expect("skill");
    std::fs::write(root.join("agents/reviewer.md"), "---\nname: reviewer\ndescription: Reviews\n---\nReview.")
        .expect("agent");
    root
}

fn model() -> ModelOption {
    ModelOption::new("claude-opus-5-5", "Opus 5.5")
}

fn backend(engine: FakeEngine) -> (LocalBackend, Receiver<()>) {
    backend_with(engine, false)
}

fn backend_with(engine: FakeEngine, memory_tool: bool) -> (LocalBackend, Receiver<()>) {
    let mut services = LocalServices::with_engine(Arc::new(engine));
    services.extensions.dirs = vec![sdk_folder()];
    services.memory = Memories::plain(Arc::new(InMemoryStore::default()));
    services.memory_tool = memory_tool;
    services.providers = services.providers.with_store(Arc::new(backend::InMemoryProviderStore::default()));
    let (woke_tx, woke) = crossbeam_channel::unbounded();
    let wake: Wake = Arc::new(move || {
        let _ = woke_tx.send(());
    });
    (LocalBackend::new(services, wake).expect("backend"), woke)
}

fn scripted() -> FakeEngine {
    FakeEngine::new(
        vec![model()],
        vec![EngineEvent::TextDelta("Hello".into()), EngineEvent::Completed { duration_ms: 10, cost_usd: None }],
    )
}

fn request() -> PromptRequest {
    PromptRequest::plain(PathBuf::from("/repo"), "hi", model())
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

fn run_ended(event: &ServerEvent) -> bool {
    matches!(event, ServerEvent::Run { event, .. } if event.is_terminal())
}

fn send(backend: &LocalBackend, command: ClientCommand) {
    backend.send(Envelope::local(command)).expect("send");
}

#[test]
fn submit_streams_run_events_and_wakes() {
    let (backend, woke) = backend(scripted());
    let run = RunId::new();
    send(&backend, ClientCommand::SubmitPrompt { run, request: request() });
    let events = collect(&backend, &woke, run_ended);
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
    send(&backend, ClientCommand::SubmitPrompt { run, request: request() });
    send(&backend, ClientCommand::CancelRun { run });
    let events = collect(&backend, &woke, run_ended);
    assert_eq!(events.last(), Some(&ServerEvent::Run { run, event: EngineEvent::Cancelled }));
}

#[test]
fn answers_reach_the_run_that_asked() {
    let ask = AgentAsk {
        id: AskId("r1".into()),
        kind: AskKind::Permission { tool: "Bash".into(), subject: "ls".into(), rememberable: false, rule: None },
    };
    let engine = FakeEngine::new(vec![model()], vec![EngineEvent::Asked(ask.clone())]);
    let (backend, woke) = backend(engine.clone());
    let run = RunId::new();
    send(&backend, ClientCommand::SubmitPrompt { run, request: request() });
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
fn queries_answer_models_skills_and_agents() {
    let (backend, woke) = backend(scripted());
    send(&backend, ClientCommand::ListModels);
    let models = collect(&backend, &woke, |event| matches!(event, ServerEvent::Models(_)));
    assert_eq!(models, vec![ServerEvent::Models(vec![model()])]);

    let dir = tempfile::tempdir().expect("tempdir");
    send(&backend, ClientCommand::ListSkills { path: dir.path().to_path_buf() });
    let skills = collect(&backend, &woke, |event| matches!(event, ServerEvent::Skills(_)));
    assert!(matches!(&skills[..], [ServerEvent::Skills(list)] if list.iter().any(|s| s.name == "motion")));

    send(&backend, ClientCommand::ListAgents { path: dir.path().to_path_buf() });
    let agents = collect(&backend, &woke, |event| matches!(event, ServerEvent::Agents(_)));
    assert!(matches!(&agents[..], [ServerEvent::Agents(list)] if list.iter().any(|a| a.name == "reviewer")));
}

#[test]
fn an_optimization_is_a_run_over_the_memory_without_the_search_tool() {
    let engine = scripted();
    let (backend, woke) = backend_with(engine.clone(), true);
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().to_path_buf();
    let fact =
        domain::MemoryEdit::Remember { kind: domain::MemoryKind::Semantic, text: "Uses wgpu".into(), tags: vec![] };
    send(&backend, ClientCommand::EditMemory { path: path.clone(), edits: vec![fact] });
    collect(&backend, &woke, |event| matches!(event, ServerEvent::Memory { .. }));

    let run = RunId::new();
    send(&backend, ClientCommand::OptimizeMemory { run, path: path.clone(), model: model() });
    collect(&backend, &woke, run_ended);
    let mut remembering = PromptRequest::plain(path.clone(), "hi", model());
    remembering.memory = Some(domain::MemoryBrief::default());
    send(&backend, ClientCommand::SubmitPrompt { run: RunId::new(), request: remembering });
    collect(&backend, &woke, run_ended);

    let [optimizing, plain] = &engine.requests()[..] else { panic!("two runs: {:?}", engine.requests()) };
    assert!(optimizing.prompt.contains("long-term memory") && optimizing.prompt.contains("[0] semantic: Uses wgpu"));
    assert!(optimizing.memory.as_ref().is_some_and(|brief| brief.optimizing && brief.recalled.is_empty()));
    assert!(optimizing.credentials.mcp.is_empty(), "no memory_search while tidying");
    assert!(!plain.memory.as_ref().is_some_and(|brief| brief.optimizing), "a client cannot ask for it");
    assert_eq!(plain.credentials.mcp.len(), 1, "an ordinary run searches memory");

    send(&backend, ClientCommand::UndoMemory { path });
    let undo =
        collect(&backend, &woke, |event| matches!(event, ServerEvent::Memory { .. } | ServerEvent::Error { .. }));
    assert!(
        matches!(undo.last(), Some(ServerEvent::Error { message }) if message.contains("nothing to undo")),
        "an optimization that changed nothing leaves nothing to undo: {undo:?}"
    );
}

#[test]
fn remote_principals_are_rejected_locally() {
    let (backend, woke) = backend(scripted());
    backend
        .send(Envelope::new(Principal::member(UserId::new(), domain::OrgId::new()), ClientCommand::ListModels))
        .expect("send");
    match &collect(&backend, &woke, |event| matches!(event, ServerEvent::Error { .. }))[..] {
        [ServerEvent::Error { message }] => assert!(message.contains("may not list_models")),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn dropping_the_backend_cancels_runs() {
    let (backend, _woke) = backend(FakeEngine::demo().with_delay(Duration::from_secs(5)));
    send(&backend, ClientCommand::SubmitPrompt { run: RunId::new(), request: request() });
    let started = Instant::now();
    drop(backend);
    assert!(started.elapsed() < Duration::from_secs(2));
}
