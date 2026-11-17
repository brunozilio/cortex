use agent_engine::fake::FakeEngine;
use app::{EngineEvent, Model, PermissionMode};
use ratatui::crossterm::event::KeyCode;

use super::super::app::App;
use super::super::tests::{client_without, press, screen, type_text, until};

fn engine() -> FakeEngine {
    let done = vec![EngineEvent::Completed { duration_ms: 1, cost_usd: None }];
    FakeEngine::new(vec![Model::new("keyed:scripted", "Scripted")], done)
}

fn outside_a_repository(folder: &std::path::Path, engine: &FakeEngine) -> App {
    let client = client_without(engine, folder, &[domain::features::MEMORY_SEARCH]);
    let mut app = App::new(client, folder.into(), None, Some(PermissionMode::Ask));
    until(&mut app, |app| app.session.is_some());
    app
}

fn send(app: &mut App, text: &str) {
    type_text(app, text);
    press(app, KeyCode::Enter);
}

fn offered(app: &mut App, text: &str) {
    send(app, text);
    until(app, |app| app.ask.is_some());
}

fn reached(app: &mut App, engine: &FakeEngine, prompts: usize) -> Vec<String> {
    until(app, |_| sent(engine).len() >= prompts);
    sent(engine)
}

fn sent(engine: &FakeEngine) -> Vec<String> {
    engine
        .requests()
        .into_iter()
        .filter(|request| request.session.command.is_none())
        .map(|request| request.prompt.text)
        .collect()
}

#[test]
fn the_first_prompt_outside_a_repository_waits_for_the_offer_and_yes_initializes_git() {
    let folder = tempfile::tempdir().expect("folder");
    let engine = engine();
    let mut app = outside_a_repository(folder.path(), &engine);

    offered(&mut app, "build a gateway");
    assert!(sent(&engine).is_empty(), "held until answered");
    assert!(screen(&app).contains("Initialize one here?"), "{}", screen(&app));

    press(&mut app, KeyCode::Enter);
    assert_eq!(reached(&mut app, &engine, 1), ["build a gateway"]);
    assert!(folder.path().join(".git").is_dir());
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

    offered(&mut app, "build a gateway");
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

    offered(&mut app, "build a gateway");
    press(&mut app, KeyCode::Char('3'));
    assert!(app.session.turned_off.contains(domain::features::GIT_OFFER));
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

#[test]
fn an_agent_question_over_the_offer_is_answered_to_the_agent_and_the_held_prompt_returns_to_the_input() {
    let folder = tempfile::tempdir().expect("folder");
    let ask = domain::AgentAsk {
        id: domain::AskId("a-1".into()),
        kind: domain::AskKind::Permission {
            tool: "Bash".into(),
            subject: "ls".into(),
            rememberable: false,
            rule: None,
        },
    };
    let script = vec![EngineEvent::Asked(ask), EngineEvent::Completed { duration_ms: 1, cost_usd: None }];
    let engine = FakeEngine::new(vec![Model::new("keyed:scripted", "Scripted")], script)
        .with_delay(std::time::Duration::from_millis(60));
    let mut app = outside_a_repository(folder.path(), &engine);

    offered(&mut app, "build a gateway");
    assert!(app.ask.as_ref().is_some_and(|pending| pending.ask.id.0 == domain::git_offer::ASK));
    app.send_as("a notice arrived".into(), "notice".into());
    until(&mut app, |app| app.ask.as_ref().is_some_and(|pending| pending.ask.id.0 == "a-1"));
    assert_eq!(app.prompt.input.text(), "build a gateway", "the held prompt is not lost");

    press(&mut app, KeyCode::Enter);
    until(&mut app, |_| !engine.answers().is_empty());
    assert_eq!(engine.answers(), [(domain::AskId("a-1".into()), app::Decision::Allow)]);
    assert!(!folder.path().join(".git").exists(), "the answer was for the agent, not the offer");
    assert_eq!(sent(&engine), ["a notice arrived"]);
}
