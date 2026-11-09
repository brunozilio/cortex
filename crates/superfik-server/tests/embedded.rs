//! The server end to end, embedded, with a scripted engine.

use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_channel::Receiver;
use superfik_backend::{Backend, InMemoryStore, LocalServices, Memories, Wake};
use superfik_core::{
    Effort, EngineEvent, EngineKind, ModelOption, OrgId, Principal, PromptRequest, RunId, UserId, WorkspaceId,
};
use superfik_engine::EngineRegistry;
use superfik_engine::fake::FakeEngine;
use superfik_protocol::{ClientCommand, Envelope, PROTOCOL_VERSION, ServerEvent};
use superfik_server::{Connection, Server};

fn model() -> ModelOption {
    ModelOption::new(EngineKind::Native, "claude-opus-5-5", "Opus 5.5")
}

fn server() -> Server {
    let engine = FakeEngine::new(
        EngineKind::Native,
        vec![model()],
        vec![EngineEvent::TextDelta("Hello".into()), EngineEvent::Completed { duration_ms: 10, cost_usd: None }],
    );
    let mut services = LocalServices::with_engines(EngineRegistry::new().with(Arc::new(engine)));
    services.memory = Memories::plain(Arc::new(InMemoryStore::default()));
    services.memory_tool = false;
    Server::start(services).expect("server")
}

fn connect(server: &Server, principal: Principal) -> (Connection, Receiver<()>) {
    let (woke_tx, woke) = crossbeam_channel::unbounded();
    let wake: Wake = Arc::new(move || {
        let _ = woke_tx.send(());
    });
    (server.connect(principal, wake), woke)
}

fn request() -> PromptRequest {
    PromptRequest {
        workspace: WorkspaceId::new(),
        cwd: "/repo".into(),
        prompt: "Build a sky".into(),
        model: model(),
        effort: Effort::High,
        output_style: None,
        safety_check: true,
        repositories: vec![],
        skills: vec![],
        disabled_skills: vec![],
        references: vec![],
        notes: vec![],
        resume: None,
        command: None,
        handoff: None,
        memory: None,
        permission: Default::default(),
        connections: vec![],
        plugins: None,
        browser: None,
        credentials: Default::default(),
    }
}

fn collect(client: &Connection, woke: &Receiver<()>, done: impl Fn(&ServerEvent) -> bool) -> Vec<ServerEvent> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut events = Vec::new();
    while Instant::now() < deadline {
        let _ = woke.recv_timeout(Duration::from_millis(50));
        while let Some(event) = client.try_recv() {
            let finished = done(&event);
            events.push(event);
            if finished {
                return events;
            }
        }
    }
    panic!("timed out; got {events:?}");
}

fn send(client: &Connection, command: ClientCommand) {
    client.send(Envelope::local(command)).expect("send");
}

#[test]
fn the_local_owner_is_welcomed_and_runs_a_prompt() {
    let server = server();
    let (client, woke) = connect(&server, Principal::LocalOwner);
    send(&client, ClientCommand::Hello { version: PROTOCOL_VERSION });
    let welcome = collect(&client, &woke, |event| matches!(event, ServerEvent::Welcome { .. }));
    assert_eq!(welcome, [ServerEvent::Welcome { version: PROTOCOL_VERSION, principal: Principal::LocalOwner }]);

    let run = RunId::new();
    send(&client, ClientCommand::SubmitPrompt { run, request: request() });
    let events =
        collect(&client, &woke, |event| matches!(event, ServerEvent::Run { event, .. } if event.is_terminal()));
    assert!(
        events.iter().any(
            |event| matches!(event, ServerEvent::Run { event: EngineEvent::TextDelta(text), .. } if text == "Hello")
        )
    );
}

#[test]
fn a_run_reaches_only_the_connection_that_started_it() {
    let server = server();
    let (starter, woke) = connect(&server, Principal::LocalOwner);
    let (other, _other_woke) = connect(&server, Principal::LocalOwner);
    send(&starter, ClientCommand::SubmitPrompt { run: RunId::new(), request: request() });
    collect(&starter, &woke, |event| matches!(event, ServerEvent::Run { event, .. } if event.is_terminal()));
    assert_eq!(other.try_recv(), None);
}

#[test]
fn a_connection_speaks_only_as_the_principal_it_was_opened_for() {
    let server = server();
    let member = Principal::member(UserId::new(), OrgId::new());
    let (client, woke) = connect(&server, member.clone());
    send(&client, ClientCommand::Hello { version: PROTOCOL_VERSION });
    let refused = collect(&client, &woke, |event| matches!(event, ServerEvent::Error { .. }));
    assert!(matches!(&refused[..], [ServerEvent::Error { message }] if message.contains("may not hello")));
}
