use agent_engine::fake::FakeEngine;
use sdk::{EngineEvent, Model};

use super::*;
use crate::tui::tests::{sdk_without, until};

fn engine() -> FakeEngine {
    let done = vec![
        EngineEvent::SessionStarted(SessionId("s".into())),
        EngineEvent::Completed { duration_ms: 1, cost_usd: None },
    ];
    FakeEngine::new(vec![Model::new("keyed:scripted", "Scripted")], done)
}

fn app(folder: &std::path::Path, engine: &FakeEngine) -> App {
    std::fs::create_dir(folder.join(".git")).expect("a repository");
    let sdk = sdk_without(engine, folder, &["memory_search"]);
    let mut app = App::new(sdk, folder.into(), None, Some(PermissionMode::Ask));
    until(&mut app, |app| app.session.is_some());
    app
}

fn asked(engine: &FakeEngine) -> Vec<(String, Option<String>)> {
    let prompts = engine.requests().into_iter().filter(|request| request.command.is_none());
    prompts.map(|request| (request.prompt, request.resume.map(|id| id.0))).collect()
}

fn prompt(session: &str, text: &str) -> Prompt {
    Prompt { session: session.into(), text: text.into() }
}

#[test]
fn a_prompt_for_the_terminal_s_own_session_is_sent_as_if_typed_there() {
    let folder = tempfile::tempdir().expect("folder");
    let engine = engine();
    let mut app = app(folder.path(), &engine);
    app.take_prompt("hi".into());
    until(&mut app, |app| !app.busy() && app.session.as_ref().and_then(sdk::Session::id).is_some());
    let id = app.session.as_ref().and_then(sdk::Session::id).expect("an id").0;

    deliver(&mut app, vec![prompt(&id, "again")], &Running::default());
    until(&mut app, |_| asked(&engine).len() == 2);
    assert_eq!(asked(&engine)[1], ("again".into(), Some(id)));
    assert!(app.transcript.entries.iter().any(|entry| matches!(entry, domain::conversation::Entry::User(text) if text == "again")));
}

#[test]
fn a_prompt_for_another_session_runs_that_session_beside_the_terminal() {
    let folder = tempfile::tempdir().expect("folder");
    let engine = engine();
    let mut app = app(folder.path(), &engine);
    let running = Running::default();

    deliver(&mut app, vec![prompt("elsewhere", "from the page")], &running);
    until(&mut app, |_| asked(&engine).len() == 1);
    assert_eq!(asked(&engine), [("from the page".into(), Some("elsewhere".into()))]);
    until(&mut app, |_| running.locked().is_empty());
    assert!(!app.busy(), "the terminal's own session stays free");
}
