use std::time::Duration;

use agent_engine::fake::FakeEngine;
use app::{Decision, EngineEvent, Model, SessionId};
use domain::{AgentAsk, AskId, AskKind};
use ratatui::crossterm::event::KeyCode;

use super::app::App;
use super::tests::{client, press, screen, type_text, until};

fn engine(script: Vec<EngineEvent>) -> FakeEngine {
    FakeEngine::new(vec![Model::new("keyed:scripted", "Scripted")], script)
}

#[test]
fn left_on_an_empty_prompt_opens_the_sessions_and_esc_goes_back() {
    let data = tempfile::tempdir().expect("data");
    let engine = engine(Vec::new());
    let mut app = App::new(client(&engine, data.path()), data.path().into(), None, None);
    until(&mut app, |app| app.session.is_some());
    press(&mut app, KeyCode::Left);
    let fleet = screen(&app);
    assert!(fleet.contains("Sessions") && fleet.contains("New session"), "{fleet}");
    press(&mut app, KeyCode::Esc);
    let back = screen(&app);
    assert!(!back.contains("Sessions"), "{back}");
    type_text(&mut app, "ab");
    press(&mut app, KeyCode::Left);
    assert!(!screen(&app).contains("Sessions"), "with text typed, left moves the cursor");
    assert_eq!(app.prompt.input.cursor(), 1);
    app.prompt.input.clear();
    type_text(&mut app, "/fleet");
    press(&mut app, KeyCode::Enter);
    assert!(screen(&app).contains("Sessions"), "{}", screen(&app));
}

fn slow_reply(text: &str) -> FakeEngine {
    let script = vec![
        EngineEvent::SessionStarted(SessionId("first".into())),
        EngineEvent::TextDelta(text.into()),
        EngineEvent::Completed { duration_ms: 5, cost_usd: None },
    ];
    engine(script).with_delay(Duration::from_millis(80))
}

#[test]
fn a_new_session_starts_while_the_one_left_keeps_working_on_its_own() {
    let data = tempfile::tempdir().expect("data");
    let engine = slow_reply("first answer");
    let mut app = App::new(client(&engine, data.path()), data.path().into(), None, None);
    until(&mut app, |app| app.session.is_some());
    type_text(&mut app, "first task");
    press(&mut app, KeyCode::Enter);
    until(&mut app, |app| app.turn.run.is_some());
    press(&mut app, KeyCode::Left);
    type_text(&mut app, "n");
    assert!(!app.busy() && !app.fleet.open, "a fresh session takes the screen");
    press(&mut app, KeyCode::Left);
    let fleet = screen(&app);
    assert!(fleet.contains("New session") && fleet.contains("first task"), "{fleet}");
    assert!(fleet.contains("working"), "{fleet}");
    until(&mut app, |app| screen(app).contains("done"));
    press(&mut app, KeyCode::Esc);
    assert!(!screen(&app).contains("first answer"), "{}", screen(&app));
}

#[test]
fn enter_on_a_session_left_behind_brings_it_back_with_what_it_did_meanwhile() {
    let data = tempfile::tempdir().expect("data");
    let engine = slow_reply("first answer");
    let mut app = App::new(client(&engine, data.path()), data.path().into(), None, None);
    until(&mut app, |app| app.session.is_some());
    type_text(&mut app, "first task");
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Left);
    type_text(&mut app, "n");
    press(&mut app, KeyCode::Left);
    until(&mut app, |app| screen(app).contains("done"));
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Enter);
    let back = screen(&app);
    assert!(back.contains("> first task") && back.contains("first answer"), "{back}");
    press(&mut app, KeyCode::Left);
    let fleet = screen(&app);
    assert!(fleet.contains("first task") && fleet.contains("New session  idle"), "{fleet}");
    assert!(!fleet.contains("done"), "seen, it is no longer news: {fleet}");
}

#[test]
fn an_earlier_session_of_this_folder_is_listed_and_enter_takes_it_up_again() {
    let data = tempfile::tempdir().expect("data");
    let engine = slow_reply("first answer");
    let mut earlier = App::new(client(&engine, data.path()), data.path().into(), None, None);
    until(&mut earlier, |app| app.session.is_some());
    type_text(&mut earlier, "earlier task");
    press(&mut earlier, KeyCode::Enter);
    until(&mut earlier, |app| !app.busy());
    earlier.client.rename_session(&SessionId("first".into()), "Earlier work").expect("rename");
    drop(earlier);
    let mut app = App::new(client(&engine, data.path()), data.path().into(), None, None);
    until(&mut app, |app| app.session.is_some());
    press(&mut app, KeyCode::Left);
    until(&mut app, |app| screen(app).contains("Earlier work"));
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Enter);
    assert!(!app.fleet.open);
    assert_eq!(app.session.title.as_deref(), Some("Earlier work"));
    assert_eq!(app.session.id(), Some(SessionId("first".into())));
    press(&mut app, KeyCode::Left);
    let fleet = screen(&app);
    assert_eq!(fleet.matches("Earlier work").count(), 1, "listed once, as the one here: {fleet}");
    assert!(!fleet.contains("New session"), "an untouched new session is not kept: {fleet}");
}

#[test]
fn a_session_left_behind_that_asks_shows_it_needs_you_and_takes_the_answer_once_entered() {
    let data = tempfile::tempdir().expect("data");
    let ask = AgentAsk {
        id: AskId("a-1".into()),
        kind: AskKind::Permission { tool: "Bash".into(), subject: "cargo test".into(), rememberable: true, rule: None },
    };
    let script = vec![
        EngineEvent::SessionStarted(SessionId("first".into())),
        EngineEvent::Asked(ask),
        EngineEvent::TextDelta("tests ran".into()),
        EngineEvent::Completed { duration_ms: 5, cost_usd: None },
    ];
    let engine = engine(script).with_delay(Duration::from_millis(60));
    let mut app = App::new(client(&engine, data.path()), data.path().into(), None, None);
    until(&mut app, |app| app.session.is_some());
    type_text(&mut app, "run the tests");
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Left);
    type_text(&mut app, "n");
    assert!(app.ask.is_none());
    press(&mut app, KeyCode::Left);
    until(&mut app, |app| screen(app).contains("needs you"));
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Enter);
    assert!(screen(&app).contains("Allow Bash: cargo test?"), "{}", screen(&app));
    type_text(&mut app, "y");
    until(&mut app, |app| !app.busy());
    assert!(screen(&app).contains("tests ran"), "{}", screen(&app));
    assert_eq!(engine.answers(), [(AskId("a-1".into()), Decision::Allow)]);
}
