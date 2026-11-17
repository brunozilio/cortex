#![allow(clippy::expect_used, reason = "a failed setup step stops the test")]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use agent_engine::fake::FakeEngine;
use app::api::features::memory::{MemoryOp, MemoryRequest};
use app::api::{ClientCommand, Envelope, FeatureError, FeatureName, FeatureRequest, Reply, RequestId, ServerEvent};
use app::{Backend, LocalBackend, LocalServices, Wake};
use crossbeam_channel::Receiver;
use domain::{AgentAsk, AskId, AskKind, Decision, EngineEvent, ModelOption, PromptRequest, RunId};
use serde_json::json;

fn project_with_a_skill_and_an_agent() -> tempfile::TempDir {
    let project = tempfile::tempdir().expect("project");
    let root = project.path().join(brand::DIR);
    std::fs::create_dir_all(root.join("skills/motion")).expect("skills");
    std::fs::create_dir_all(root.join("agents")).expect("agents");
    std::fs::write(root.join("skills/motion/SKILL.md"), "---\nname: motion\ndescription: Motion\n---\nMove.")
        .expect("skill");
    std::fs::write(root.join("agents/reviewer.md"), "---\nname: reviewer\ndescription: Reviews\n---\nReview.")
        .expect("agent");
    project
}

fn model() -> ModelOption {
    ModelOption::new("claude-opus-5-5", "Opus 5.5")
}

fn backend(engine: FakeEngine) -> (LocalBackend, Receiver<()>) {
    backend_with(engine, false)
}

fn backend_with(engine: FakeEngine, memory_tool: bool) -> (LocalBackend, Receiver<()>) {
    let mut services = LocalServices::with_engine(Arc::new(engine));
    if !memory_tool {
        services = services.without(domain::features::MEMORY_SEARCH);
    }
    services.providers = services.providers.with_store(Arc::new(app::InMemoryProviderStore::default()));
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

fn send(backend: &LocalBackend, command: ClientCommand) -> RequestId {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let id = RequestId(NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
    backend.send(Envelope::local(id, command)).expect("send");
    id
}

fn answer(backend: &LocalBackend, woke: &Receiver<()>, command: ClientCommand) -> Reply {
    let asked = send(backend, command);
    let events = collect(backend, woke, |event| matches!(event, ServerEvent::Reply { id, .. } if *id == asked));
    match events.into_iter().last() {
        Some(ServerEvent::Reply { reply, .. }) => reply,
        other => panic!("expected an answer, got {other:?}"),
    }
}

#[test]
fn submit_streams_run_events_and_wakes() {
    let (backend, woke) = backend(scripted());
    let run = RunId::new();
    let submitted = send(&backend, ClientCommand::SubmitPrompt { run, request: request() });
    let events = collect(&backend, &woke, run_ended);
    let accepted = ServerEvent::Reply { id: submitted, reply: Reply::Done };
    let of_the_run: Vec<_> = events.iter().filter(|event| **event != accepted).cloned().collect();
    assert_eq!(
        of_the_run,
        vec![
            ServerEvent::Run { run, event: EngineEvent::TextDelta("Hello".into()) },
            ServerEvent::Run { run, event: EngineEvent::Completed { duration_ms: 10, cost_usd: None } },
        ]
    );
}

#[test]
fn a_prompt_for_a_run_already_in_progress_is_refused_to_the_request_that_sent_it() {
    let (backend, woke) = backend(FakeEngine::demo().with_delay(Duration::from_millis(200)));
    let run = RunId::new();
    let first = send(&backend, ClientCommand::SubmitPrompt { run, request: request() });
    let second = send(&backend, ClientCommand::SubmitPrompt { run, request: request() });
    let events = collect(&backend, &woke, |event| matches!(event, ServerEvent::Reply { id, .. } if *id == second));
    assert!(events.contains(&ServerEvent::Reply { id: first, reply: Reply::Done }), "{events:?}");
    assert!(
        matches!(events.last(), Some(ServerEvent::Reply { reply: Reply::Error { message }, .. }) if message.contains("already in progress")),
        "{events:?}"
    );
    send(&backend, ClientCommand::CancelRun { run });
}

#[test]
fn every_command_that_says_nothing_else_is_acknowledged() {
    let (backend, woke) = backend(scripted());
    let unknown = RunId::new();
    for command in [
        ClientCommand::SaveSettings { settings: json!({ "model": "m" }) },
        ClientCommand::CancelRun { run: unknown },
        ClientCommand::SteerRun { run: unknown, text: "use tabs".into() },
        ClientCommand::UnsteerRun { run: unknown },
        ClientCommand::BackgroundRun { run: unknown },
        ClientCommand::CancelRequest { request: RequestId(u64::MAX) },
    ] {
        let name = command.name();
        assert_eq!(answer(&backend, &woke, command), Reply::Done, "{name}");
    }
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
    assert_eq!(answer(&backend, &woke, ClientCommand::ListModels), Reply::Models(vec![model()]));

    let dir = project_with_a_skill_and_an_agent();
    let skills = answer(&backend, &woke, ClientCommand::ListSkills { path: dir.path().to_path_buf() });
    assert!(matches!(&skills, Reply::Skills(list) if list.iter().any(|s| s.name == "motion")));

    let agents = answer(&backend, &woke, ClientCommand::ListAgents { path: dir.path().to_path_buf() });
    assert!(matches!(&agents, Reply::Agents(list) if list.iter().any(|a| a.name == "reviewer")));
}

#[test]
fn an_optimization_is_a_run_over_the_memory_without_the_search_tool() {
    let engine = scripted();
    let (backend, woke) = backend_with(engine.clone(), true);
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().to_path_buf();
    let fact = domain::MemoryEdit::Remember {
        kind: domain::MemoryKind::Semantic,
        text: "Uses wgpu".into(),
        tags: vec![],
        touches: vec![],
    };
    answer(&backend, &woke, memory(&path, MemoryOp::Edit { edits: vec![fact] }));

    let run = RunId::new();
    send(&backend, memory(&path, MemoryOp::Optimize { run, model: model() }));
    collect(&backend, &woke, run_ended);
    let mut remembering = PromptRequest::plain(path.clone(), "hi", model());
    remembering.host.memory = Some(domain::MemoryBrief::default());
    send(&backend, ClientCommand::SubmitPrompt { run: RunId::new(), request: remembering });
    collect(&backend, &woke, run_ended);

    let [optimizing, plain] = &engine.requests()[..] else { panic!("two runs: {:?}", engine.requests()) };
    assert!(
        optimizing.prompt.text.contains("long-term memory")
            && optimizing.prompt.text.contains("[0] semantic, from the user: Uses wgpu")
    );
    assert!(optimizing.host.memory.as_ref().is_some_and(|brief| brief.optimizing && brief.recalled.is_empty()));
    assert!(optimizing.host.credentials.mcp.is_empty(), "no memory_search while tidying");
    assert!(!plain.host.memory.as_ref().is_some_and(|brief| brief.optimizing), "a client cannot ask for it");
    assert!(plain.host.credentials.mcp.is_empty(), "memory that fits in the prompt is not searched");

    let undo = answer(&backend, &woke, memory(&path, MemoryOp::Undo));
    assert!(
        matches!(&undo, Reply::FeatureFailed(FeatureError::Failed { feature: FeatureName::Memory, message })
            if message.contains("nothing to undo")),
        "an optimization that changed nothing leaves nothing to undo: {undo:?}"
    );
}

fn memory(path: &std::path::Path, op: MemoryOp) -> ClientCommand {
    ClientCommand::Feature(FeatureRequest::Memory(MemoryRequest { path: path.to_path_buf(), op }))
}

#[test]
fn dropping_the_backend_cancels_runs() {
    let (backend, _woke) = backend(FakeEngine::demo().with_delay(Duration::from_secs(5)));
    send(&backend, ClientCommand::SubmitPrompt { run: RunId::new(), request: request() });
    let started = Instant::now();
    drop(backend);
    assert!(started.elapsed() < Duration::from_secs(2));
}
