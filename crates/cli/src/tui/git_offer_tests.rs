use agent_engine::fake::FakeEngine;
use ratatui::crossterm::event::KeyCode;
use sdk::{EngineEvent, Model, PermissionMode};

use super::super::app::App;
use super::super::tests::{press, screen, sdk_without, type_text, until};

fn engine() -> FakeEngine {
    let done = vec![EngineEvent::Completed { duration_ms: 1, cost_usd: None }];
    FakeEngine::new(vec![Model::new("keyed:scripted", "Scripted")], done)
}

fn outside_a_repository(folder: &std::path::Path, engine: &FakeEngine) -> App {
    let sdk = sdk_without(engine, folder, &["memory_search"]);
    let mut app = App::new(sdk, folder.into(), None, Some(PermissionMode::Ask));
    until(&mut app, |app| app.session.is_some());
    app
}

fn send(app: &mut App, text: &str) {
    type_text(app, text);
    press(app, KeyCode::Enter);
}

fn reached(app: &mut App, engine: &FakeEngine, prompts: usize) -> Vec<String> {
    until(app, |_| sent(engine).len() >= prompts);
    sent(engine)
}

fn sent(engine: &FakeEngine) -> Vec<String> {
    engine.requests().into_iter().filter(|request| request.command.is_none()).map(|request| request.prompt).collect()
}

#[test]
fn the_first_prompt_outside_a_repository_waits_for_the_offer_and_yes_initializes_git() {
    let folder = tempfile::tempdir().expect("folder");
    let engine = engine();
    let mut app = outside_a_repository(folder.path(), &engine);

    send(&mut app, "build a gateway");
    assert!(sent(&engine).is_empty(), "held until answered");
    assert!(screen(&app).contains("Initialize one here?"), "{}", screen(&app));

    press(&mut app, KeyCode::Enter);
    assert!(folder.path().join(".git").is_dir());
    assert_eq!(reached(&mut app, &engine, 1), ["build a gateway"]);
    until(&mut app, |app| !app.busy());

    send(&mut app, "and tests");
    assert!(app.ask.is_none(), "offered once");
    until(&mut app, |_| sent(&engine).iter().any(|prompt| prompt == "and tests"));
}

#[test]
fn declining_the_offer_sends_the_prompt_without_a_repository_and_does_not_offer_again() {
    let folder = tempfile::tempdir().expect("folder");
    let engine = engine();
    let mut app = outside_a_repository(folder.path(), &engine);

    send(&mut app, "build a gateway");
    press(&mut app, KeyCode::Esc);
    assert!(!folder.path().join(".git").exists());
    assert_eq!(reached(&mut app, &engine, 1), ["build a gateway"]);
    until(&mut app, |app| !app.busy());

    send(&mut app, "and tests");
    assert!(app.ask.is_none());
}

#[test]
fn never_offering_again_turns_the_feature_off() {
    let folder = tempfile::tempdir().expect("folder");
    let engine = engine();
    let mut app = outside_a_repository(folder.path(), &engine);

    send(&mut app, "build a gateway");
    press(&mut app, KeyCode::Char('3'));
    assert!(app.turned_off.contains(domain::features::GIT_OFFER));
    assert!(!folder.path().join(".git").exists());
    assert_eq!(reached(&mut app, &engine, 1), ["build a gateway"]);
}

#[test]
fn a_folder_that_is_a_repository_gets_no_offer() {
    let folder = tempfile::tempdir().expect("folder");
    let made = std::process::Command::new("git").args(["init", "-q"]).current_dir(folder.path()).status().expect("git");
    assert!(made.success());
    let engine = engine();
    let mut app = outside_a_repository(folder.path(), &engine);

    send(&mut app, "build a gateway");
    assert!(app.ask.is_none());
    assert_eq!(reached(&mut app, &engine, 1), ["build a gateway"]);
}
