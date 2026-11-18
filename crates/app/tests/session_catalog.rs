#![allow(clippy::expect_used, reason = "a failed setup step stops the test")]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use agent_engine::fake::FakeEngine;
use app::api::features::sessions::{SessionsReply, SessionsRequest};
use app::api::{ClientCommand, Envelope, FeatureReply, FeatureRequest, Reply, RequestId, ServerEvent};
use app::{Backend, LocalBackend, LocalServices, Wake};
use crossbeam_channel::Receiver;
use domain::catalog::Listed;
use domain::conversation::SessionStatus;
use domain::{AgentAsk, AskId, AskKind, EngineEvent, ModelOption, PromptRequest, RunId, SessionCommand, SessionId};
use serde_json::{Value, json};

fn model() -> ModelOption {
    ModelOption::new("claude-opus-5-5", "Opus 5.5")
}

fn backend(script: Vec<EngineEvent>) -> (LocalBackend, Receiver<()>) {
    backend_keeping(script, &json!({}))
}

fn backend_keeping(script: Vec<EngineEvent>, settings: &Value) -> (LocalBackend, Receiver<()>) {
    let data = tempfile::tempdir().expect("data").keep();
    let services = LocalServices::with_engine(Arc::new(FakeEngine::new(vec![model()], script)))
        .kept_in(&data)
        .without(domain::features::MEMORY_SEARCH);
    services.settings.save(settings).expect("settings");
    let (woke_tx, woke) = crossbeam_channel::unbounded();
    let wake: Wake = Arc::new(move || {
        let _ = woke_tx.send(());
    });
    (LocalBackend::new(services, wake).expect("backend"), woke)
}

fn until(backend: &LocalBackend, woke: &Receiver<()>, done: impl Fn(&ServerEvent) -> bool) -> ServerEvent {
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        let _ = woke.recv_timeout(Duration::from_millis(50));
        while let Some(event) = backend.try_recv() {
            if done(&event) {
                return event;
            }
        }
    }
    panic!("timed out");
}

fn run(backend: &LocalBackend, woke: &Receiver<()>, request: PromptRequest, stop: impl Fn(&EngineEvent) -> bool) {
    let run = RunId::new();
    backend.send(Envelope::local(RequestId(1), ClientCommand::SubmitPrompt { run, request })).expect("send");
    until(backend, woke, |event| matches!(event, ServerEvent::Run { event, .. } if stop(event)));
}

fn sessions(backend: &LocalBackend, woke: &Receiver<()>, request: SessionsRequest) -> SessionsReply {
    let asked = RequestId(2);
    backend.send(Envelope::local(asked, ClientCommand::Feature(FeatureRequest::Sessions(request)))).expect("send");
    match until(backend, woke, |event| matches!(event, ServerEvent::Reply { id, .. } if *id == asked)) {
        ServerEvent::Reply { reply: Reply::Feature(FeatureReply::Sessions(reply)), .. } => reply,
        other => panic!("{other:?}"),
    }
}

fn listed(backend: &LocalBackend, woke: &Receiver<()>) -> Vec<Listed> {
    match sessions(backend, woke, SessionsRequest::List) {
        SessionsReply::Listed(listed) => listed,
        other => panic!("{other:?}"),
    }
}

fn s1() -> SessionId {
    SessionId("s1".into())
}

fn request() -> PromptRequest {
    PromptRequest::plain(PathBuf::from("/repo"), "hi", model())
}

#[test]
fn a_run_lists_its_session_and_how_it_ended() {
    let (backend, woke) = backend(vec![
        EngineEvent::SessionStarted(SessionId("s1".into())),
        EngineEvent::TextDelta("Hello".into()),
        EngineEvent::Completed { duration_ms: 10, cost_usd: None },
    ]);
    run(&backend, &woke, request(), EngineEvent::is_terminal);

    let rows = listed(&backend, &woke);
    assert_eq!(rows.len(), 1, "{rows:?}");
    let row = &rows[0];
    assert_eq!((&row.session, row.cwd.as_path()), (&s1(), std::path::Path::new("/repo")));
    assert_eq!((row.status, row.unread), (SessionStatus::Done, true));

    sessions(&backend, &woke, SessionsRequest::Rename { session: s1(), title: "Greeting".into() });
    sessions(&backend, &woke, SessionsRequest::Seen { session: s1() });
    let row = listed(&backend, &woke).remove(0);
    assert_eq!((row.title.as_deref(), row.unread), (Some("Greeting"), false));

    sessions(&backend, &woke, SessionsRequest::Delete { session: s1() });
    assert!(listed(&backend, &woke).is_empty());
}

#[test]
fn a_session_waiting_for_an_answer_is_listed_as_needing_you() {
    let ask = AgentAsk { id: AskId("a".into()), kind: AskKind::Questions(Vec::new()) };
    let (backend, woke) = backend(vec![EngineEvent::SessionStarted(SessionId("s2".into())), EngineEvent::Asked(ask)]);
    run(&backend, &woke, request(), |event| matches!(event, EngineEvent::Asked(_)));
    let row = listed(&backend, &woke).remove(0);
    assert_eq!((row.status, row.unread), (SessionStatus::NeedsYou, true));
}

#[test]
fn a_side_chore_is_not_a_session_of_its_own() {
    let (backend, woke) = backend(vec![
        EngineEvent::SessionStarted(SessionId("chore".into())),
        EngineEvent::Completed { duration_ms: 1, cost_usd: None },
    ]);
    let mut titling = request();
    titling.session.command = Some(SessionCommand::Title);
    run(&backend, &woke, titling, EngineEvent::is_terminal);
    assert!(listed(&backend, &woke).is_empty());
}

#[test]
fn the_conversations_kept_in_the_settings_are_listed_too() {
    let settings = json!({"conversations": {"/repo": [{"session": "old", "title": "Earlier", "at": 5}]}});
    let (backend, woke) = backend_keeping(Vec::new(), &settings);
    assert_eq!(listed(&backend, &woke)[0].title.as_deref(), Some("Earlier"));
}
