#![allow(clippy::expect_used, reason = "a failed setup step stops the test")]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use agent_engine::fake::FakeEngine;
use backend::{Backend, LocalBackend, LocalServices, Wake};
use crossbeam_channel::Receiver;
use domain::{AgentAsk, AskId, AskKind, EngineEvent, ModelOption, PromptRequest, RunId, SessionCommand, SessionId};
use protocol::{ClientCommand, Envelope, ServerEvent};
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
        .without("memory_search");
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
    backend.send(Envelope::local(ClientCommand::SubmitPrompt { run, request })).expect("send");
    until(backend, woke, |event| matches!(event, ServerEvent::Run { event, .. } if stop(event)));
}

fn sessions(backend: &LocalBackend, woke: &Receiver<()>, input: Value) -> Value {
    backend.send(Envelope::local(ClientCommand::Feature { feature: "sessions".into(), input })).expect("send");
    match until(backend, woke, |event| matches!(event, ServerEvent::Feature { feature, .. } if feature == "sessions")) {
        ServerEvent::Feature { output, .. } => output.expect("the sessions feature answers"),
        other => panic!("{other:?}"),
    }
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

    let listed = sessions(&backend, &woke, json!({ "op": "list" }));
    assert_eq!(listed["sessions"].as_array().map(Vec::len), Some(1), "{listed}");
    let row = &listed["sessions"][0];
    assert_eq!((row["session"].as_str(), row["cwd"].as_str()), (Some("s1"), Some("/repo")));
    assert_eq!((row["status"].as_str(), row["unread"].as_bool()), (Some("done"), Some(true)));

    sessions(&backend, &woke, json!({ "op": "rename", "session": "s1", "title": "Greeting" }));
    sessions(&backend, &woke, json!({ "op": "seen", "session": "s1" }));
    let row = sessions(&backend, &woke, json!({ "op": "list" }))["sessions"][0].clone();
    assert_eq!((row["title"].as_str(), row["unread"].as_bool()), (Some("Greeting"), Some(false)));

    sessions(&backend, &woke, json!({ "op": "archive", "session": "s1", "archived": true }));
    assert_eq!(sessions(&backend, &woke, json!({ "op": "list" }))["sessions"][0]["archived"], true);
    sessions(&backend, &woke, json!({ "op": "delete", "session": "s1" }));
    assert_eq!(sessions(&backend, &woke, json!({ "op": "list" }))["sessions"], json!([]));
}

#[test]
fn a_session_waiting_for_an_answer_is_listed_as_needing_you() {
    let ask = AgentAsk { id: AskId("a".into()), kind: AskKind::Questions(Vec::new()) };
    let (backend, woke) = backend(vec![EngineEvent::SessionStarted(SessionId("s2".into())), EngineEvent::Asked(ask)]);
    run(&backend, &woke, request(), |event| matches!(event, EngineEvent::Asked(_)));
    let row = sessions(&backend, &woke, json!({ "op": "list" }))["sessions"][0].clone();
    assert_eq!((row["status"].as_str(), row["unread"].as_bool()), (Some("needs_you"), Some(true)));
}

#[test]
fn a_side_chore_is_not_a_session_of_its_own() {
    let (backend, woke) = backend(vec![
        EngineEvent::SessionStarted(SessionId("chore".into())),
        EngineEvent::Completed { duration_ms: 1, cost_usd: None },
    ]);
    let titling = PromptRequest { command: Some(SessionCommand::Title), ..request() };
    run(&backend, &woke, titling, EngineEvent::is_terminal);
    assert_eq!(sessions(&backend, &woke, json!({ "op": "list" }))["sessions"], json!([]));
}

#[test]
fn the_conversations_kept_in_the_settings_are_listed_too() {
    let settings = json!({"conversations": {"/repo": [{"session": "old", "title": "Earlier", "at": 5}]}});
    let (backend, woke) = backend_keeping(Vec::new(), &settings);
    let listed = sessions(&backend, &woke, json!({ "op": "list" }));
    assert_eq!(listed["sessions"][0]["title"], "Earlier");
}
