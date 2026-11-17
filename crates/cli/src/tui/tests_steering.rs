use std::time::Duration;

use agent_engine::fake::FakeEngine;
use app::{EngineEvent, Model};
use domain::{SessionId, ToolCall};
use ratatui::crossterm::event::KeyCode;

use super::app::App;
use super::tests::{client, press, screen, type_text, until};

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
    let model = Model::new("keyed:scripted", "Scripted");
    let engine = FakeEngine::new(vec![model], script()).with_delay(Duration::from_millis(60));
    let mut app = App::new(client(&engine, data.path()), data.path().into(), None, None);
    until(&mut app, |app| app.session.is_some());
    type_text(&mut app, "read the code");
    press(&mut app, KeyCode::Enter);
    until(&mut app, |app| app.turn.run.is_some());

    type_text(&mut app, "also check the docs");
    press(&mut app, KeyCode::Enter);
    assert!(screen(&app).contains("also check the docs  (queued)"), "{}", screen(&app));
    until(&mut app, |app| app.turn.queued.is_empty());
    let handed = screen(&app);
    assert!(handed.contains("> also check the docs") && !handed.contains("(queued)"), "{handed}");
    assert!(app.busy(), "handed over within the run");

    until(&mut app, |app| !app.busy());
    assert_eq!(engine.steers(), ["also check the docs"]);
    assert_eq!(prompts(&engine), ["read the code"], "no second prompt");
}

#[test]
fn up_takes_back_a_message_not_handed_over_yet_to_edit_it() {
    let data = tempfile::tempdir().expect("data");
    let model = Model::new("keyed:scripted", "Scripted");
    let engine = FakeEngine::new(vec![model], script()).with_delay(Duration::from_millis(60)).holding_steers();
    let mut app = App::new(client(&engine, data.path()), data.path().into(), None, None);
    until(&mut app, |app| app.session.is_some());
    type_text(&mut app, "read the code");
    press(&mut app, KeyCode::Enter);
    until(&mut app, |app| app.turn.run.is_some());
    type_text(&mut app, "also check the docs");
    press(&mut app, KeyCode::Enter);

    press(&mut app, KeyCode::Up);
    until(&mut app, |app| app.turn.queued.is_empty());
    assert_eq!(app.prompt.input.text(), "also check the docs");
    assert!(!screen(&app).contains("(queued)"), "{}", screen(&app));
    until(&mut app, |app| !app.busy());
    assert_eq!(prompts(&engine), ["read the code"], "taken back, never sent");
}

#[test]
fn the_bell_rings_when_the_agent_finishes_while_the_terminal_is_elsewhere() {
    let data = tempfile::tempdir().expect("data");
    let model = Model::new("keyed:scripted", "Scripted");
    let engine = FakeEngine::new(vec![model], script());
    let mut app = App::new(client(&engine, data.path()), data.path().into(), None, None);
    until(&mut app, |app| app.session.is_some());
    type_text(&mut app, "read the code");
    press(&mut app, KeyCode::Enter);
    until(&mut app, |app| !app.busy());
    assert!(!app.ui.bell, "watching: no bell");
    app.ui.focused = false;
    type_text(&mut app, "again");
    press(&mut app, KeyCode::Enter);
    until(&mut app, |app| !app.busy());
    assert!(app.ui.bell);
}

#[test]
fn a_long_paste_shows_folded_and_goes_whole() {
    let data = tempfile::tempdir().expect("data");
    let engine = FakeEngine::new(vec![Model::new("keyed:scripted", "Scripted")], script());
    let mut app = App::new(client(&engine, data.path()), data.path().into(), None, None);
    until(&mut app, |app| app.session.is_some());
    type_text(&mut app, "explain ");
    let log = "line 1\nline 2\nline 3\nline 4\nline 5\nline 6";
    super::keys::paste(&mut app, log);
    assert_eq!(app.prompt.input.text(), "explain [Pasted text #1 +6 lines]");
    press(&mut app, KeyCode::Enter);
    until(&mut app, |_| !prompts(&engine).is_empty());
    assert_eq!(prompts(&engine), [format!("explain {log}")]);
    assert!(screen(&app).contains("> explain [Pasted text #1 +6 lines]"), "{}", screen(&app));
}

#[test]
fn ctrl_r_finds_an_earlier_prompt_and_esc_twice_opens_rewind() {
    use ratatui::crossterm::event::{KeyEvent, KeyModifiers};
    let data = tempfile::tempdir().expect("data");
    let engine = FakeEngine::new(vec![Model::new("keyed:scripted", "Scripted")], script());
    let mut app = App::new(client(&engine, data.path()), data.path().into(), None, None);
    until(&mut app, |app| app.session.is_some());
    for prompt in ["run the tests", "explain main"] {
        type_text(&mut app, prompt);
        press(&mut app, KeyCode::Enter);
        until(&mut app, |app| !app.busy());
    }
    let ctrl = |c| KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL);
    super::keys::key(&mut app, ctrl('r'));
    type_text(&mut app, "tests");
    assert!(screen(&app).contains("search 'tests': run the tests"), "{}", screen(&app));
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.prompt.input.text(), "run the tests");
    assert!(app.overlays.search.is_none());

    app.prompt.input.clear();
    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::Esc);
    assert!(
        screen(&app).contains("Nothing to rewind to in this chat yet."),
        "esc twice on an empty prompt opens /rewind (no checkpoints in this script): {}",
        screen(&app)
    );
}

fn prompts(engine: &FakeEngine) -> Vec<String> {
    engine
        .requests()
        .into_iter()
        .filter(|request| request.session.command.is_none())
        .map(|request| request.prompt.text)
        .collect()
}

#[test]
fn a_message_the_engine_never_took_is_sent_as_the_next_prompt() {
    let data = tempfile::tempdir().expect("data");
    let model = Model::new("keyed:scripted", "Scripted");
    let engine = FakeEngine::new(vec![model], script()).with_delay(Duration::from_millis(30)).without_steering();
    let mut app = App::new(client(&engine, data.path()), data.path().into(), None, None);
    until(&mut app, |app| app.session.is_some());
    type_text(&mut app, "read the code");
    press(&mut app, KeyCode::Enter);
    until(&mut app, |app| app.turn.run.is_some());

    type_text(&mut app, "also check the docs");
    press(&mut app, KeyCode::Enter);
    until(&mut app, |_| prompts(&engine).len() == 2);
    assert_eq!(prompts(&engine), ["read the code", "also check the docs"]);
}

#[test]
fn the_arrows_walk_past_a_recalled_command_until_it_is_edited() {
    let data = tempfile::tempdir().expect("data");
    let engine = FakeEngine::new(vec![Model::new("keyed:scripted", "Scripted")], Vec::new());
    let mut app = App::new(client(&engine, data.path()), data.path().into(), None, None);
    until(&mut app, |app| app.session.is_some());
    app.prompt.input.set_history(vec!["first".into(), "/help".into(), "last".into()]);

    press(&mut app, KeyCode::Up);
    press(&mut app, KeyCode::Up);
    assert_eq!(app.prompt.input.text(), "/help");
    assert!(!screen(&app).contains("Show the commands and keys"), "{}", screen(&app));
    press(&mut app, KeyCode::Up);
    assert_eq!(app.prompt.input.text(), "first");
    press(&mut app, KeyCode::Down);
    assert_eq!(app.prompt.input.text(), "/help");
    press(&mut app, KeyCode::Down);
    assert_eq!(app.prompt.input.text(), "last");

    press(&mut app, KeyCode::Up);
    press(&mut app, KeyCode::Backspace);
    assert_eq!(app.prompt.input.text(), "/hel");
    assert!(screen(&app).contains("Show the commands and keys"), "{}", screen(&app));
    press(&mut app, KeyCode::Up);
    assert_eq!(app.prompt.input.text(), "/hel", "an edited command is completed again");
}
