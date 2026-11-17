use std::time::{Duration, Instant};

use agent_engine::fake::FakeEngine;
use app::{EngineEvent, Model, PermissionMode};
use domain::SessionId;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::app::{App, Msg};
use super::tests::{SETTLES_WITHIN, client, client_without, press, screen, type_text, until};
use super::transcript::Entry;

fn engine() -> FakeEngine {
    let script = vec![
        EngineEvent::SessionStarted(SessionId("s".into())),
        EngineEvent::TextDelta("Done.".into()),
        EngineEvent::Completed { duration_ms: 1, cost_usd: None },
    ];
    FakeEngine::new(vec![Model::new("keyed:scripted", "Scripted")], script)
}

fn send(app: &mut App, text: &str) {
    type_text(app, text);
    press(app, KeyCode::Enter);
}

fn prompts(engine: &FakeEngine) -> Vec<String> {
    engine
        .requests()
        .into_iter()
        .filter(|request| request.session.command.is_none())
        .map(|request| request.prompt.text)
        .collect()
}

fn until_the_worker_reports(app: &mut App, is_its_result: impl Fn(&Msg) -> bool) {
    let deadline = Instant::now() + SETTLES_WITHIN;
    loop {
        assert!(Instant::now() < deadline, "the worker never reported in {SETTLES_WITHIN:?}");
        let Ok(msg) = app.rx.recv_timeout(Duration::from_millis(50)) else { continue };
        let reported = is_its_result(&msg);
        app.message(msg);
        if reported {
            return;
        }
    }
}

fn said(app: &App) -> Vec<&str> {
    app.transcript
        .entries
        .iter()
        .filter_map(|entry| match entry {
            Entry::User(text) => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn esc_while_a_prompt_reads_its_files_takes_it_back_and_the_late_result_sends_nothing() {
    let data = tempfile::tempdir().expect("data");
    std::fs::write(data.path().join("notes.txt"), "the notes").expect("write");
    let engine = engine();
    let mut app = App::new(client(&engine, data.path()), data.path().into(), None, None);
    until(&mut app, |app| app.session.is_some());
    send(&mut app, "hello");
    until(&mut app, |app| !app.busy() && prompts(&engine).len() == 1);

    send(&mut app, "read @notes.txt");
    send(&mut app, "and then this");
    assert!(app.busy() && app.turn.run.is_none(), "the worker has not reported yet");

    press(&mut app, KeyCode::Esc);

    assert!(!app.busy(), "interrupting ends the wait");
    assert_eq!(app.prompt.input.text(), "read @notes.txt\n\nand then this", "what was typed is back in the input");
    assert!(!screen(&app).contains("(queued)"), "{}", screen(&app));

    until_the_worker_reports(&mut app, |msg| matches!(msg, Msg::Prepared { .. }));
    assert!(!app.busy() && app.turn.run.is_none(), "a result nobody waits for starts no run");
    assert_eq!(prompts(&engine), ["hello"]);
    assert_eq!(said(&app), ["hello"]);

    app.prompt.input.clear();
    send(&mut app, "read @notes.txt again");
    until(&mut app, |app| !app.busy() && prompts(&engine).len() == 2);
    assert!(prompts(&engine)[1].contains("again") && prompts(&engine)[1].contains("the notes"));
}

#[test]
fn ctrl_c_while_the_first_prompt_waits_for_the_git_check_takes_it_back_and_the_check_runs_again_next_time() {
    let folder = tempfile::tempdir().expect("folder");
    let engine = engine();
    let client = client_without(&engine, folder.path(), &[domain::features::MEMORY_SEARCH]);
    let mut app = App::new(client, folder.path().into(), None, Some(PermissionMode::Ask));
    until(&mut app, |app| app.session.is_some());

    send(&mut app, "build a gateway");
    assert!(app.busy() && app.turn.run.is_none(), "held while the folder is checked");

    super::keys::key(&mut app, KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));

    assert!(!app.busy());
    assert_eq!(app.prompt.input.text(), "build a gateway");
    until_the_worker_reports(&mut app, |msg| matches!(msg, Msg::GitOffer { .. }));
    assert!(app.ask.is_none() && !app.busy(), "the late answer offers nothing and sends nothing");
    assert!(prompts(&engine).is_empty(), "{:?}", prompts(&engine));

    press(&mut app, KeyCode::Enter);
    until(&mut app, |app| app.ask.is_some());
    assert!(screen(&app).contains("Initialize one here?"), "the offer was not used up: {}", screen(&app));
}
