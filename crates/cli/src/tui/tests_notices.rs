use std::time::{Duration, Instant};

use agent_engine::fake::FakeEngine;
use app::api::Notices;
use app::{EngineEvent, Model};
use domain::SessionId;

use super::app::{App, Msg};
use super::tests::{screen, sdk, until};
use super::transcript::Entry;

fn script() -> Vec<EngineEvent> {
    vec![
        EngineEvent::SessionStarted(SessionId("s".into())),
        EngineEvent::TextDelta("Done.".into()),
        EngineEvent::Completed { duration_ms: 1, cost_usd: None },
    ]
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
fn a_harness_notice_is_shown_to_the_user_and_never_sent_to_the_agent() {
    let data = tempfile::tempdir().expect("data");
    let engine = FakeEngine::new(vec![Model::new("keyed:scripted", "Scripted")], script());
    let mut app = App::new(sdk(&engine, data.path()), data.path().into(), None, None);
    until(&mut app, |app| app.session.is_some());
    let told = "keys cannot be kept on this machine: no data folder".to_owned();
    app.message(Msg::Notices(Notices { for_user: vec![told], for_agent: Vec::new() }));
    settle(&mut app);
    assert!(prompts(&engine).is_empty(), "the model was told: {:?}", prompts(&engine));
    assert!(!app.busy(), "and no run was started for it");
    assert!(screen(&app).contains("keys cannot be kept on this machine"), "{}", screen(&app));
}

fn settle(app: &mut App) {
    let settled = Instant::now() + Duration::from_millis(400);
    while Instant::now() < settled {
        if let Ok(msg) = app.rx.recv_timeout(Duration::from_millis(20)) {
            app.message(msg);
        }
    }
}

#[test]
fn a_database_that_cannot_be_opened_is_said_once_on_screen_even_while_a_run_goes_on() {
    let data = tempfile::tempdir().expect("data");
    std::fs::create_dir(data.path().join("app.db")).expect("a folder where the database should be");
    let slow = FakeEngine::new(vec![Model::new("keyed:scripted", "Scripted")], script())
        .with_delay(Duration::from_millis(150));
    let mut app = App::new(sdk(&slow, data.path()), data.path().into(), None, None);
    until(&mut app, |app| app.session.is_some());
    app.send("hello".into());
    assert!(app.busy());

    super::notices::poll(&mut app);
    until(&mut app, |app| screen(app).contains("could not be opened"));
    until(&mut app, |app| !app.busy());
    app.notices = super::every::Every::second();
    super::notices::poll(&mut app);
    settle(&mut app);

    let said = app
        .transcript
        .entries
        .iter()
        .filter(|entry| matches!(entry, Entry::Notice(text) if text.starts_with("The session list and memory could not be opened (")))
        .count();
    assert_eq!(said, 1, "{}", screen(&app));
    assert_eq!(prompts(&slow), ["hello"], "only what the user typed reached the model");
}

#[test]
fn events_that_reached_the_inbox_are_sent_as_a_turn_when_idle() {
    let data = tempfile::tempdir().expect("data");
    let engine = FakeEngine::new(vec![Model::new("keyed:scripted", "Scripted")], script());
    let mut app = App::new(sdk(&engine, data.path()), data.path().into(), None, None);
    until(&mut app, |app| app.session.is_some());
    let notice = domain::inbox::Notice::new("monitor bash_4", "ERROR: disk full");
    app.message(Msg::Notices(Notices { for_user: Vec::new(), for_agent: vec![notice] }));
    until(&mut app, |app| !app.busy() && !prompts(&engine).is_empty());
    let sent = &prompts(&engine)[0];
    assert!(sent.contains("<event source=\"monitor bash_4\">") && sent.contains("ERROR: disk full"), "{sent}");
    assert!(screen(&app).contains("[monitor bash_4] ERROR: disk full"), "{}", screen(&app));
}
