use agent_engine::fake::FakeEngine;
use app::{EngineEvent, Model, PermissionMode};
use ratatui::crossterm::event::KeyCode;
use serde_json::json;

use super::app::App;
use super::tests::{client, press, type_text, until};

fn engine() -> FakeEngine {
    let done = vec![EngineEvent::Completed { duration_ms: 1, cost_usd: None }];
    FakeEngine::new(vec![Model::new("keyed:scripted", "Scripted")], done)
}

fn started(engine: &FakeEngine, data: &std::path::Path) -> App {
    let mut app = App::new(client(engine, data), data.into(), None, None);
    until(&mut app, |app| app.session.is_some());
    app
}

#[test]
fn a_reload_keeps_the_mode_and_effort_of_the_live_session() {
    let data = tempfile::tempdir().expect("data");
    let engine = engine();
    let mut app = started(&engine, data.path());
    app.set_mode(PermissionMode::Plan);
    app.set_effort_from(Some(app::Effort::High));

    let models = vec![Model::new("keyed:scripted", "Scripted")];
    app.ready(Ok(models), &json!({ "mode": "bypass", "effort": "auto" }));
    assert_eq!(app.mode(), PermissionMode::Plan, "what the session runs with is what shows");
    assert_eq!(app.effort_label(), "high");

    type_text(&mut app, "go");
    press(&mut app, KeyCode::Enter);
    until(&mut app, |_| !engine.requests().is_empty());
    let sent = engine.requests().remove(0);
    assert_eq!((sent.policy.permission, sent.session.effort), (PermissionMode::Plan, app::Effort::High));
}

#[test]
fn the_first_load_still_takes_the_saved_mode_and_effort() {
    let data = tempfile::tempdir().expect("data");
    let engine = engine();
    let mut app = App::new(client(&engine, data.path()), data.path().into(), None, None);
    let models = vec![Model::new("keyed:scripted", "Scripted")];
    app.ready(Ok(models), &json!({ "mode": "plan", "effort": "max" }));
    assert_eq!((app.mode(), app.effort_label()), (PermissionMode::Plan, "max"));
}

#[test]
fn each_row_of_the_mode_picker_sets_the_mode_it_names() {
    let data = tempfile::tempdir().expect("data");
    let engine = engine();
    let mut app = started(&engine, data.path());
    for (row, mode) in PermissionMode::ALL.into_iter().enumerate() {
        type_text(&mut app, "/mode");
        press(&mut app, KeyCode::Enter);
        let picker = app.overlays.picker.as_mut().expect("the mode picker");
        assert_eq!(picker.items.len(), PermissionMode::ALL.len());
        assert_eq!(picker.items[row].0, domain::command::mode_label(mode));
        assert!(!picker.items[row].1.is_empty(), "every mode says what it does");
        picker.cursor = row;
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.mode(), mode);
    }
}

#[test]
fn the_esc_hint_on_an_empty_prompt_promises_the_rewind_the_second_esc_opens() {
    let data = tempfile::tempdir().expect("data");
    let engine = engine();
    let mut app = started(&engine, data.path());
    press(&mut app, KeyCode::Esc);
    let hinted = super::tests::screen(&app);
    assert!(hinted.contains("Press Esc again to go back to an earlier prompt"), "{hinted}");
    assert!(!hinted.contains("clear the prompt"), "{hinted}");

    type_text(&mut app, "draft");
    press(&mut app, KeyCode::Esc);
    assert!(super::tests::screen(&app).contains("Press Esc again to clear the prompt"));
    press(&mut app, KeyCode::Esc);
    assert!(app.prompt.input.is_empty());
}

#[test]
fn clear_forgets_the_images_and_pastes_of_the_conversation_before() {
    let data = tempfile::tempdir().expect("data");
    let png = b"\x89PNG\r\n\x1a\n pixels";
    std::fs::write(data.path().join("shot.png"), png).expect("png");
    let engine = engine();
    let mut app = started(&engine, data.path());
    let long = "a\nb\nc\nd\ne\nf";
    super::keys::paste(&mut app, "shot.png");
    super::keys::paste(&mut app, long);
    assert_eq!(app.prompt.input.text(), "[Image #1] [Pasted text #1 +6 lines]");
    press(&mut app, KeyCode::Enter);
    let prompts =
        || engine.requests().into_iter().filter(|request| request.session.command.is_none()).collect::<Vec<_>>();
    until(&mut app, |app| !app.busy() && prompts().len() == 1);
    assert_eq!(prompts()[0].prompt.images.len(), 1);

    type_text(&mut app, "/clear");
    press(&mut app, KeyCode::Enter);
    type_text(&mut app, "about [Image #1] and [Pasted text #1 +6 lines]");
    press(&mut app, KeyCode::Enter);
    until(&mut app, |app| !app.busy() && prompts().len() == 2);
    let sent = prompts().remove(1);
    assert!(sent.prompt.images.is_empty(), "the earlier conversation's image is not sent again");
    assert_eq!(sent.prompt.text, "about [Image #1] and [Pasted text #1 +6 lines]");
}
