use agent_engine::fake::FakeEngine;
use app::{EngineEvent, Model};
use domain::SessionId;
use ratatui::crossterm::event::KeyCode;

use super::app::App;
use super::tests::{client, press, screen, type_text, until};

#[test]
fn an_added_folder_goes_with_every_prompt_even_after_new() {
    let data = tempfile::tempdir().expect("data");
    let shared = tempfile::tempdir().expect("shared");
    let script = vec![
        EngineEvent::SessionStarted(SessionId("s".into())),
        EngineEvent::Completed { duration_ms: 1, cost_usd: None },
    ];
    let engine = FakeEngine::new(vec![Model::new("keyed:scripted", "Scripted")], script);
    let mut app = App::new(client(&engine, data.path()), data.path().into(), None, None);
    until(&mut app, |app| app.session.is_some());
    let real = shared.path().canonicalize().expect("real");

    type_text(&mut app, &format!("/add-dir {}", shared.path().display()));
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.session.added_dirs(), std::slice::from_ref(&real));
    type_text(&mut app, "/new");
    press(&mut app, KeyCode::Enter);
    type_text(&mut app, "look at the shared code");
    press(&mut app, KeyCode::Enter);
    until(&mut app, |_| engine.requests().iter().any(|request| request.session.command.is_none()));
    let sent = engine.requests().into_iter().find(|request| request.session.command.is_none()).expect("the prompt");
    assert_eq!(sent.session.add_dirs, [real]);
}

#[test]
fn permissions_lists_the_project_s_rules() {
    let data = tempfile::tempdir().expect("data");
    let project = tempfile::tempdir().expect("project");
    std::fs::create_dir_all(project.path().join(brand::DIR)).expect("dir");
    std::fs::write(
        project.path().join(concat!(brand::dir!(), "/settings.json")),
        r#"{"permissions": {"deny": ["Read(.env)"], "ask": ["Bash(git push:*)"]}}"#,
    )
    .expect("write");
    let engine = FakeEngine::new(vec![Model::new("keyed:scripted", "Scripted")], Vec::new());
    let mut app = App::new(client(&engine, data.path()), project.path().into(), None, None);
    until(&mut app, |app| app.session.is_some());
    type_text(&mut app, "/permissions");
    press(&mut app, KeyCode::Enter);
    until(&mut app, |app| screen(app).contains("Permission rules"));
    let shown = screen(&app);
    assert!(shown.contains("deny   Read(.env)") && shown.contains("ask    Bash(git push:*)"), "{shown}");
}

#[test]
fn an_at_mention_completes_a_project_file_and_sends_its_text() {
    let data = tempfile::tempdir().expect("data");
    let project = tempfile::tempdir().expect("project");
    std::fs::create_dir_all(project.path().join("src")).expect("src");
    std::fs::write(project.path().join("src/main.rs"), "fn main() {}").expect("write");
    let init = std::process::Command::new("git").arg("init").arg("-q").current_dir(project.path()).status();
    assert!(init.is_ok_and(|status| status.success()), "git is needed for this test");
    let script = vec![EngineEvent::Completed { duration_ms: 1, cost_usd: None }];
    let engine = FakeEngine::new(vec![Model::new("keyed:scripted", "Scripted")], script);
    let mut app = App::new(client(&engine, data.path()), project.path().into(), None, None);
    until(&mut app, |app| app.session.is_some());

    type_text(&mut app, "explain @mai");
    until(&mut app, |app| app.catalog.files.as_ref().is_some_and(|files| !files.is_empty()));
    assert!(screen(&app).contains("@src/main.rs"), "{}", screen(&app));
    press(&mut app, KeyCode::Tab);
    assert_eq!(app.prompt.input.text(), "explain @src/main.rs ");
    press(&mut app, KeyCode::Enter);
    until(&mut app, |_| engine.requests().iter().any(|r| r.session.command.is_none()));
    let sent = engine.requests().into_iter().find(|r| r.session.command.is_none()).expect("sent").prompt.text;
    assert!(sent.contains("<file path=\"src/main.rs\">\nfn main() {}\n</file>"), "{sent}");
}

#[test]
fn resume_picks_an_earlier_conversation_of_the_folder() {
    let data = tempfile::tempdir().expect("data");
    let project = tempfile::tempdir().expect("project");
    let script = vec![EngineEvent::Completed { duration_ms: 1, cost_usd: None }];
    let engine = FakeEngine::new(vec![Model::new("keyed:scripted", "Scripted")], script);
    let client = client(&engine, data.path());
    let earlier = serde_json::json!({
        "session": "old", "title": "Login page", "at": super::conversations::now() - 7_200,
    });
    let settings = serde_json::json!({"conversations": { project.path().to_string_lossy(): [earlier] }});
    client.save_settings(settings).expect("saved");
    let mut app = App::new(client, project.path().into(), None, None);
    until(&mut app, |app| app.session.is_some());

    type_text(&mut app, "/resume");
    press(&mut app, KeyCode::Enter);
    until(&mut app, |app| app.overlays.picker.is_some());
    assert!(screen(&app).contains("Login page") && screen(&app).contains("2h ago"), "{}", screen(&app));
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.session.title.as_deref(), Some("Login page"));
    type_text(&mut app, "go on");
    press(&mut app, KeyCode::Enter);
    until(&mut app, |_| engine.requests().iter().any(|r| r.session.command.is_none()));
    let sent = engine.requests().into_iter().find(|r| r.session.command.is_none()).expect("sent");
    assert_eq!(sent.session.resume, Some(SessionId("old".into())));
}

#[test]
fn resume_offers_a_session_another_client_ran_in_the_folder() {
    let data = tempfile::tempdir().expect("data");
    let project = tempfile::tempdir().expect("project");
    let script = vec![
        EngineEvent::SessionStarted(SessionId("web-1".into())),
        EngineEvent::Completed { duration_ms: 1, cost_usd: None },
    ];
    let model = Model::new("keyed:scripted", "Scripted");
    let engine = FakeEngine::new(vec![model.clone()], script);
    let client = client(&engine, data.path());
    let elsewhere = client.sibling().expect("sibling");
    let id = SessionId("web-1".into());
    elsewhere.session(project.path(), model).send("hi").expect("run").text().expect("ended");
    elsewhere.rename_session(&id, "From the browser").expect("renamed");

    let mut app = App::new(client, project.path().into(), None, None);
    until(&mut app, |app| app.session.is_some());
    type_text(&mut app, "/resume");
    press(&mut app, KeyCode::Enter);
    until(&mut app, |app| app.overlays.picker.is_some());
    assert!(screen(&app).contains("From the browser"), "{}", screen(&app));
}

#[test]
fn the_name_a_conversation_gets_is_kept_where_every_client_lists_it() {
    let data = tempfile::tempdir().expect("data");
    let script = vec![
        EngineEvent::SessionStarted(SessionId("s".into())),
        EngineEvent::Chore(domain::ChoreAnswer::Title("Greeting".into())),
        EngineEvent::Completed { duration_ms: 1, cost_usd: None },
    ];
    let engine = FakeEngine::new(vec![Model::new("keyed:scripted", "Scripted")], script);
    let client = client(&engine, data.path());
    let mut app = App::new(client.clone(), data.path().into(), None, None);
    until(&mut app, |app| app.session.is_some());

    type_text(&mut app, "say hi");
    press(&mut app, KeyCode::Enter);
    until(&mut app, |app| app.session.title.as_deref() == Some("Greeting"));

    until(&mut app, |_| client.sessions().expect("listed").iter().any(|row| row.title.as_deref() == Some("Greeting")));
}

#[test]
fn the_output_style_picked_goes_with_the_next_prompt() {
    let data = tempfile::tempdir().expect("data");
    let script = vec![EngineEvent::Completed { duration_ms: 1, cost_usd: None }];
    let engine = FakeEngine::new(vec![Model::new("keyed:scripted", "Scripted")], script);
    let mut app = App::new(client(&engine, data.path()), data.path().into(), None, None);
    until(&mut app, |app| app.session.is_some());
    type_text(&mut app, "/output-style");
    press(&mut app, KeyCode::Enter);
    assert!(screen(&app).contains("explanatory"), "{}", screen(&app));
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Enter);
    type_text(&mut app, "hi");
    press(&mut app, KeyCode::Enter);
    until(&mut app, |_| engine.requests().iter().any(|r| r.session.command.is_none()));
    let sent = engine.requests().into_iter().find(|r| r.session.command.is_none()).expect("sent");
    assert_eq!(sent.session.output_style.as_deref(), Some("concise"));
}

#[test]
fn a_missing_folder_is_refused() {
    let data = tempfile::tempdir().expect("data");
    let engine = FakeEngine::new(vec![Model::new("keyed:scripted", "Scripted")], Vec::new());
    let mut app = App::new(client(&engine, data.path()), data.path().into(), None, None);
    until(&mut app, |app| app.session.is_some());
    type_text(&mut app, "/add-dir nowhere/at/all");
    press(&mut app, KeyCode::Enter);
    assert!(screen(&app).contains("No folder at"), "{}", screen(&app));
    assert!(app.session.added_dirs().is_empty());
}
