//! Messages typed while the agent works: handed to it mid-run, or sent as
//! the next prompt when the engine could not take them.

use std::time::Duration;

use ratatui::crossterm::event::KeyCode;
use superfik_core::{SessionId, ToolCall};
use superfik_native::fake::FakeEngine;
use superfik_sdk::{EngineEvent, Model};

use super::app::App;
use super::tests::{press, screen, superfik, type_text, until};

fn script() -> Vec<EngineEvent> {
    vec![
        EngineEvent::SessionStarted(SessionId("s".into())),
        EngineEvent::ToolStarted(ToolCall::new("t1", "Read", "src/main.rs")),
        EngineEvent::ToolFinished { id: "t1".into() },
        EngineEvent::ToolStarted(ToolCall::new("t2", "Read", "src/lib.rs")),
        EngineEvent::ToolFinished { id: "t2".into() },
        EngineEvent::TextDelta("Done.".into()),
        EngineEvent::Completed { duration_ms: 1, cost_usd: None },
    ]
}

#[test]
fn a_message_typed_while_the_agent_works_reaches_it_in_the_same_run() {
    let data = tempfile::tempdir().expect("data");
    let model = Model::new("anthropic:scripted", "Scripted");
    let engine = FakeEngine::new(vec![model], script()).with_delay(Duration::from_millis(60));
    let mut app = App::new(superfik(&engine, data.path()), data.path().into(), None, None);
    until(&mut app, |app| app.session.is_some());
    type_text(&mut app, "read the code");
    press(&mut app, KeyCode::Enter);
    until(&mut app, |app| app.busy());

    type_text(&mut app, "also check the docs");
    press(&mut app, KeyCode::Enter);
    assert!(screen(&app).contains("also check the docs  (queued)"), "{}", screen(&app));
    until(&mut app, |app| app.queued.is_empty());
    let handed = screen(&app);
    assert!(handed.contains("> also check the docs") && !handed.contains("(queued)"), "{handed}");
    assert!(app.busy(), "handed over within the run");

    until(&mut app, |app| !app.busy());
    assert_eq!(engine.steers(), ["also check the docs"]);
    assert_eq!(prompts(&engine), ["read the code"], "no second prompt");
}

/// The prompts sent, without the side calls the chat makes by itself.
fn prompts(engine: &FakeEngine) -> Vec<String> {
    engine.requests().into_iter().filter(|request| request.command.is_none()).map(|request| request.prompt).collect()
}

#[test]
fn a_message_the_engine_never_took_is_sent_as_the_next_prompt() {
    let data = tempfile::tempdir().expect("data");
    let model = Model::new("anthropic:scripted", "Scripted");
    let engine = FakeEngine::new(vec![model], script()).with_delay(Duration::from_millis(30)).without_steering();
    let mut app = App::new(superfik(&engine, data.path()), data.path().into(), None, None);
    until(&mut app, |app| app.session.is_some());
    type_text(&mut app, "read the code");
    press(&mut app, KeyCode::Enter);
    until(&mut app, |app| app.busy());

    type_text(&mut app, "also check the docs");
    press(&mut app, KeyCode::Enter);
    until(&mut app, |_| prompts(&engine).len() == 2);
    assert_eq!(prompts(&engine), ["read the code", "also check the docs"]);
}
