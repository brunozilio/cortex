use std::time::{Duration, Instant};

use agent_engine::fake::FakeEngine;
use app::{EngineEvent, Model};
use domain::{FailureKind, SessionId};
use ratatui::crossterm::event::KeyCode;

use super::app::App;
use super::tests::{client, press, screen, type_text, until};

fn failing(kind: FailureKind) -> FakeEngine {
    let script = vec![
        EngineEvent::SessionStarted(SessionId("s".into())),
        EngineEvent::Failure(kind),
        EngineEvent::Failed("rate limited: overloaded".into()),
    ];
    FakeEngine::new(vec![Model::new("keyed:scripted", "Scripted")], script)
}

fn failed_once(engine: &FakeEngine, data: &std::path::Path) -> App {
    let mut app = App::new(client(engine, data), data.into(), None, None);
    until(&mut app, |app| app.session.is_some());
    type_text(&mut app, "build it");
    press(&mut app, KeyCode::Enter);
    until(&mut app, |app| !app.busy() && !engine.requests().is_empty());
    app
}

#[test]
fn an_unavailable_provider_is_tried_once_more_after_a_visible_countdown() {
    let data = tempfile::tempdir().expect("data");
    let engine = failing(FailureKind::Unavailable);
    let mut app = failed_once(&engine, data.path());
    assert!(app.turn.auto_retry.is_some());
    assert!(screen(&app).contains("Retrying in 30s"), "{}", screen(&app));

    app.turn.auto_retry = Some(Instant::now() - Duration::from_secs(1));
    super::auto_retry::tick(&mut app);
    until(&mut app, |app| !app.busy() && engine.requests().len() == 2);
    assert_eq!(engine.requests()[1].prompt.text, "build it");
    assert!(app.turn.auto_retry.is_none(), "a second failure is not retried on its own");
}

#[test]
fn esc_cancels_the_countdown_and_other_failures_never_start_one() {
    let data = tempfile::tempdir().expect("data");
    let engine = failing(FailureKind::Unavailable);
    let mut app = failed_once(&engine, data.path());
    press(&mut app, KeyCode::Esc);
    assert!(app.turn.auto_retry.is_none());
    assert!(!screen(&app).contains("Retrying in"));

    let other = tempfile::tempdir().expect("data");
    let engine = failing(FailureKind::Other);
    let app = failed_once(&engine, other.path());
    assert!(app.turn.auto_retry.is_none(), "only a provider that was unavailable is retried");
}
