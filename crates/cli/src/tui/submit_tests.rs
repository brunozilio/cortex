use agent_engine::fake::FakeEngine;
use app::{EngineEvent, Model, PermissionMode};
use ratatui::crossterm::event::KeyCode;

use super::super::app::App;
use super::super::tests::{client, press, type_text, until};
use super::super::transcript::Entry;

fn ready(data: &std::path::Path, engine: &FakeEngine) -> App {
    let mut app = App::new(client(engine, data), data.into(), None, Some(PermissionMode::Ask));
    until(&mut app, |app| app.session.is_some() && !app.catalog.skills.is_empty());
    app
}

fn engine() -> FakeEngine {
    let done = vec![EngineEvent::Completed { duration_ms: 1, cost_usd: None }];
    FakeEngine::new(vec![Model::new("keyed:scripted", "Scripted")], done)
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
fn effort_auto_lets_each_request_pick_its_model_and_effort_until_a_level_is_set() {
    let data = tempfile::tempdir().expect("data");
    let engine = engine();
    let mut app = ready(data.path(), &engine);
    let notice = submit(&mut app, "/effort auto");
    assert!(notice.contains("auto"), "{notice}");
    for text in ["fix it", "/effort high", "and again"] {
        type_text(&mut app, text);
        press(&mut app, KeyCode::Enter);
        until(&mut app, |app| !app.busy());
    }
    let picks: Vec<bool> = engine
        .requests()
        .into_iter()
        .filter(|request| request.session.command.is_none())
        .map(|request| request.session.auto_pick)
        .collect();
    assert_eq!(picks, [true, false]);
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
    assert_eq!(app.prompt.input.text(), "/mode sideways", "kept to fix");
    assert!(engine.requests().is_empty(), "nothing reached the agent");
}

#[test]
fn skills_and_paths_still_reach_the_agent() {
    let data = tempfile::tempdir().expect("data");
    let engine = engine();
    let mut app = ready(data.path(), &engine);
    let skill = app.catalog.skills[0].0.clone();
    for text in [format!("/{skill} src"), "/etc/hosts has a typo?".to_owned()] {
        type_text(&mut app, &text);
        press(&mut app, KeyCode::Enter);
        until(&mut app, |app| !app.busy());
    }
    let sent: Vec<String> = engine.requests().into_iter().map(|request| request.prompt.text).collect();
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

#[test]
fn a_mistyped_command_is_refused_instead_of_starting_a_session() {
    let data = tempfile::tempdir().expect("data");
    let engine = engine();
    let mut app = ready(data.path(), &engine);
    assert!(submit(&mut app, "/mode'l").contains("Did you mean /model?"));
    assert_eq!(app.prompt.input.text(), "/mode'l", "the typo stays to fix");
    app.prompt.input.set("");
    assert!(submit(&mut app, "/").contains("/help"));
    assert!(engine.requests().is_empty(), "nothing reached the agent");
}

#[test]
fn branch_alone_lists_the_prompts_to_branch_from_instead_of_forking() {
    let data = tempfile::tempdir().expect("data");
    let engine = engine();
    let mut app = ready(data.path(), &engine);
    let notice = submit(&mut app, "/branch");
    assert_eq!(notice, "Nothing to branch from in this chat yet.");
    assert!(engine.requests().is_empty(), "nothing was forked");
}

#[test]
fn no_feature_command_takes_the_name_of_a_built_in_command() {
    for (names, _) in super::super::features::commands() {
        for name in names {
            let taken = domain::Command::HELP.iter().any(|(known, _)| known.eq_ignore_ascii_case(name));
            assert!(!taken, "/{name} is a built-in command: the feature's command would never run");
        }
    }
}
