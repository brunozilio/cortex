use agent_engine::fake::FakeEngine;
use ratatui::crossterm::event::KeyCode;
use sdk::{EngineEvent, Model, PermissionMode};

use super::super::app::App;
use super::super::tests::{press, sdk, type_text, until};
use super::super::transcript::Entry;

fn ready(data: &std::path::Path, engine: &FakeEngine) -> App {
    let mut app = App::new(sdk(engine, data), data.into(), None, Some(PermissionMode::Ask));
    until(&mut app, |app| app.session.is_some() && !app.skills.is_empty());
    app
}

fn engine() -> FakeEngine {
    let done = vec![EngineEvent::Completed { duration_ms: 1, cost_usd: None }];
    FakeEngine::new(vec![Model::new("anthropic:scripted", "Scripted")], done)
}

fn submit(app: &mut App, text: &str) -> String {
    type_text(app, text);
    press(app, KeyCode::Enter);
    match app.transcript.entries.last() {
        Some(Entry::Notice(text)) => text.clone(),
        other => panic!("no notice: {other:?}"),
    }
}

#[test]
fn a_mistyped_command_is_not_sent_and_the_nearest_is_offered() {
    let data = tempfile::tempdir().expect("data");
    let engine = engine();
    let mut app = ready(data.path(), &engine);
    let notice = submit(&mut app, "/modle");
    assert!(notice.contains("/modle") && notice.contains("/model"), "{notice}");
    assert!(engine.requests().is_empty(), "nothing reached the agent");
}

#[test]
fn a_known_command_used_wrongly_shows_how_to_use_it() {
    let data = tempfile::tempdir().expect("data");
    let engine = engine();
    let mut app = ready(data.path(), &engine);
    let notice = submit(&mut app, "/mode sideways");
    assert!(notice.contains("/mode") && notice.contains("ask, edits, plan or bypass"), "{notice}");
    assert_eq!(app.input.text(), "/mode sideways", "kept to fix");
    assert!(engine.requests().is_empty(), "nothing reached the agent");
}

#[test]
fn skills_and_paths_still_reach_the_agent() {
    let data = tempfile::tempdir().expect("data");
    let engine = engine();
    let mut app = ready(data.path(), &engine);
    let skill = app.skills[0].0.clone();
    for text in [format!("/{skill} src"), "/etc/hosts has a typo?".to_owned()] {
        type_text(&mut app, &text);
        press(&mut app, KeyCode::Enter);
        until(&mut app, |app| !app.busy());
    }
    let sent: Vec<String> = engine.requests().into_iter().map(|request| request.prompt).collect();
    assert!(sent.iter().any(|prompt| prompt.starts_with(&format!("/{skill}"))), "{sent:?}");
    assert!(sent.iter().any(|prompt| prompt.starts_with("/etc/hosts")), "{sent:?}");
}

#[test]
fn help_on_one_command_shows_just_that_command() {
    let data = tempfile::tempdir().expect("data");
    let engine = engine();
    let mut app = ready(data.path(), &engine);
    let notice = submit(&mut app, "/help effort");
    assert!(notice.contains("/effort") && notice.contains("xhigh"), "{notice}");
    assert!(!notice.contains("/diff"), "{notice}");
}
