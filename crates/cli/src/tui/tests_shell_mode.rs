use agent_engine::fake::FakeEngine;
use ratatui::crossterm::event::KeyCode;
use sdk::{EngineEvent, Model};

use super::app::App;
use super::tests::{press, screen, sdk, type_text, until};

fn app(data: &std::path::Path) -> (App, FakeEngine) {
    let done = vec![EngineEvent::Completed { duration_ms: 1, cost_usd: None }];
    let engine = FakeEngine::new(vec![Model::new("keyed:scripted", "Scripted")], done);
    let mut app = App::new(sdk(&engine, data), data.into(), None, None);
    until(&mut app, |app| app.session.is_some());
    (app, engine)
}

#[test]
fn a_bang_on_an_empty_prompt_switches_to_shell_mode_and_backspace_leaves_it() {
    let data = tempfile::tempdir().expect("data");
    let (mut app, _) = app(data.path());
    type_text(&mut app, "!");
    assert!(app.shell && app.input.is_empty(), "the bang is the mode, not text");
    assert!(screen(&app).contains("shell command"), "{}", screen(&app));
    press(&mut app, KeyCode::Backspace);
    assert!(!app.shell);
    type_text(&mut app, "wow!");
    assert_eq!((app.shell, app.input.text()), (false, "wow!"), "a bang inside text is text");
}

#[test]
fn a_command_in_shell_mode_runs_and_the_prompt_goes_back_to_normal() {
    let data = tempfile::tempdir().expect("data");
    let (mut app, _) = app(data.path());
    type_text(&mut app, "!echo from the shell");
    assert_eq!(app.input.text(), "echo from the shell");
    press(&mut app, KeyCode::Enter);
    until(&mut app, |app| screen(app).contains("from the shell") && !app.shell_context.is_empty());
    assert!(!app.shell);
    assert_eq!(app.input.history().last().map(String::as_str), Some("!echo from the shell"));
}

#[test]
fn esc_leaves_shell_mode_and_a_pasted_bang_enters_it() {
    let data = tempfile::tempdir().expect("data");
    let (mut app, _) = app(data.path());
    type_text(&mut app, "!");
    press(&mut app, KeyCode::Esc);
    assert!(!app.shell);
    super::keys::paste(&mut app, "!ls -la");
    assert_eq!((app.shell, app.input.text()), (true, "ls -la"));
}
