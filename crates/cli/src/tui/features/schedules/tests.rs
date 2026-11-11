use std::time::Duration;

use agent_engine::fake::FakeEngine;
use domain::SessionId;
use domain::schedule::{Scheduled, When};
use ratatui::crossterm::event::KeyCode;
use sdk::{EngineEvent, Model};

use super::arrived;
use crate::tui::app::App;
use crate::tui::tests::{press, screen, sdk, type_text, until};

fn script() -> Vec<EngineEvent> {
    vec![
        EngineEvent::SessionStarted(SessionId("s".into())),
        EngineEvent::TextDelta("Done.".into()),
        EngineEvent::Completed { duration_ms: 1, cost_usd: None },
    ]
}

fn prompts(engine: &FakeEngine) -> Vec<String> {
    engine.requests().into_iter().filter(|request| request.command.is_none()).map(|request| request.prompt).collect()
}

fn due(id: &str, prompt: &str) -> Scheduled {
    Scheduled { id: id.into(), prompt: prompt.into(), when: When::At(0), next: 0 }
}

fn app(engine: &FakeEngine, data: &std::path::Path) -> App {
    let mut app = App::new(sdk(engine, data), data.into(), None, None);
    until(&mut app, |app| app.session.is_some());
    app
}

#[test]
fn a_due_prompt_is_sent_while_the_chat_is_idle() {
    let data = tempfile::tempdir().expect("data");
    let engine = FakeEngine::new(vec![Model::new("anthropic:scripted", "Scripted")], script());
    let mut app = app(&engine, data.path());
    arrived(&mut app, vec![due("cron_1", "check the build")]);
    until(&mut app, |app| !app.busy() && !prompts(&engine).is_empty());
    assert_eq!(prompts(&engine), ["check the build"]);
    assert!(screen(&app).contains("Scheduled (cron_1)"), "{}", screen(&app));
}

#[test]
fn a_prompt_due_while_the_agent_works_waits_for_the_run_to_end() {
    let data = tempfile::tempdir().expect("data");
    let engine = FakeEngine::new(vec![Model::new("anthropic:scripted", "Scripted")], script())
        .with_delay(Duration::from_millis(40));
    let mut app = app(&engine, data.path());
    type_text(&mut app, "first");
    press(&mut app, KeyCode::Enter);
    until(&mut app, |app| app.busy());
    arrived(&mut app, vec![due("cron_2", "later")]);
    assert_eq!(app.queued, ["later"]);
    until(&mut app, |app| prompts(&engine).len() == 2 && !app.busy());
    assert_eq!(prompts(&engine), ["first", "later"]);
    assert!(engine.steers().is_empty(), "never pushed into the running turn");
}

#[test]
fn loop_asks_the_agent_to_schedule_the_task() {
    let data = tempfile::tempdir().expect("data");
    let engine = FakeEngine::new(vec![Model::new("anthropic:scripted", "Scripted")], script());
    let mut app = app(&engine, data.path());
    type_text(&mut app, "/loop 5m check the deploy");
    press(&mut app, KeyCode::Enter);
    until(&mut app, |app| !prompts(&engine).is_empty() && !app.busy());
    let sent = &prompts(&engine)[0];
    assert!(sent.contains("CronCreate") && sent.contains("*/5 * * * *"), "{sent}");
    assert!(screen(&app).contains("/loop 5m check the deploy"), "{}", screen(&app));

    type_text(&mut app, "/loop");
    press(&mut app, KeyCode::Enter);
    assert!(screen(&app).contains("/loop [5m|2h|1d] <task>"), "{}", screen(&app));
}

