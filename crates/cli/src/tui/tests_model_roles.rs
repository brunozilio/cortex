use agent_engine::fake::FakeEngine;
use app::{EngineEvent, Model};
use domain::SessionId;
use domain::model_roles::ModelRole;
use ratatui::crossterm::event::KeyCode;

use super::app::App;
use super::tests::{client, press, screen, type_text, until};

const MAIN: &str = "keyed:main";
const FAST: &str = "keyed:fast";

fn engine() -> FakeEngine {
    FakeEngine::new(
        vec![Model::new(MAIN, "Main"), Model::new(FAST, "Fast")],
        vec![
            EngineEvent::SessionStarted(SessionId("s".into())),
            EngineEvent::TextDelta("ok".into()),
            EngineEvent::Completed { duration_ms: 1, cost_usd: None },
        ],
    )
}

fn started(engine: &FakeEngine, data: &std::path::Path) -> App {
    let mut app = App::new(client(engine, data), data.into(), None, None);
    until(&mut app, |app| app.session.is_some());
    app
}

fn send(app: &mut App, text: &str) {
    type_text(app, text);
    press(app, KeyCode::Enter);
    until(app, |app| !app.turn.busy());
}

#[test]
fn a_role_set_by_name_reaches_the_next_run_and_is_remembered() {
    let data = tempfile::tempdir().expect("data");
    let engine = engine();
    let mut app = started(&engine, data.path());
    send(&mut app, "/model explore fast");
    assert!(screen(&app).contains("explore: Fast"), "{}", screen(&app));
    send(&mut app, "look around");
    assert_eq!(engine.requests().last().expect("run").session.roles.get(ModelRole::Explore), Some(FAST));
    until(&mut app, |app| app.client.settings().is_ok_and(|saved| saved["roles"]["explore"] == FAST));
    let mut again = started(&engine, data.path());
    send(&mut again, "again");
    assert_eq!(engine.requests().last().expect("run").session.roles.get(ModelRole::Explore), Some(FAST));
}

#[test]
fn the_role_picker_offers_the_session_model_first_and_sets_the_chosen_one() {
    let data = tempfile::tempdir().expect("data");
    let engine = engine();
    let mut app = started(&engine, data.path());
    send(&mut app, "/model plan");
    let shown = screen(&app);
    assert!(shown.contains("Model for plan") && shown.contains("Session model"), "{shown}");
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.session.roles().get(ModelRole::Plan), Some(FAST));
    send(&mut app, "/model plan default");
    assert_eq!(app.session.roles().get(ModelRole::Plan), None);
}

#[test]
fn model_roles_lists_every_role_and_what_it_uses() {
    let data = tempfile::tempdir().expect("data");
    let engine = engine();
    let mut app = started(&engine, data.path());
    send(&mut app, "/model review fast");
    send(&mut app, "/model roles");
    let shown = screen(&app);
    for expected in ["plan", "explore", "small", "advisor", "review: Fast", "session model"] {
        assert!(shown.contains(expected), "{expected}:\n{shown}");
    }
}

#[test]
fn an_unknown_model_for_a_role_is_refused() {
    let data = tempfile::tempdir().expect("data");
    let engine = engine();
    let mut app = started(&engine, data.path());
    send(&mut app, "/model small nope");
    assert!(screen(&app).contains("No model named nope"), "{}", screen(&app));
    assert_eq!(app.session.roles().get(ModelRole::Small), None);
}

#[test]
fn fallbacks_set_by_name_reach_the_next_run_and_are_remembered() {
    let data = tempfile::tempdir().expect("data");
    let engine = engine();
    let mut app = started(&engine, data.path());
    send(&mut app, "/model fallbacks fast main");
    assert!(screen(&app).contains("Fallbacks: Fast → Main"), "{}", screen(&app));
    send(&mut app, "go");
    assert_eq!(engine.requests().last().expect("run").session.fallbacks, [FAST, MAIN]);
    until(&mut app, |app| app.client.settings().is_ok_and(|saved| saved["fallbacks"][0] == FAST));
    send(&mut app, "/model fallbacks off");
    send(&mut app, "again");
    assert!(engine.requests().last().expect("run").session.fallbacks.is_empty());
    send(&mut app, "/model fallbacks nope");
    assert!(screen(&app).contains("No model named nope"), "{}", screen(&app));
}
