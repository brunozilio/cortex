use std::time::Duration;

use agent_engine::fake::FakeEngine;
use app::{EngineEvent, Model};
use domain::SessionId;
use ratatui::crossterm::event::KeyCode;

use super::app::App;
use super::tests::{client, press, screen, type_text, until};
use super::transcript::Entry;

fn script() -> Vec<EngineEvent> {
    vec![
        EngineEvent::SessionStarted(SessionId("s".into())),
        EngineEvent::TextDelta("Done.".into()),
        EngineEvent::Completed { duration_ms: 1, cost_usd: None },
    ]
}

fn started(engine: &FakeEngine, data: &std::path::Path) -> App {
    let mut app = App::new(client(engine, data), data.into(), None, None);
    until(&mut app, |app| app.session.is_some());
    app
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

#[test]
fn a_prompt_typed_while_the_one_before_reads_its_files_does_not_overtake_it() {
    let data = tempfile::tempdir().expect("data");
    std::fs::write(data.path().join("notes.txt"), "the notes").expect("write");
    let engine =
        FakeEngine::new(vec![Model::new("keyed:scripted", "Scripted")], script()).with_delay(Duration::from_millis(80));
    let mut app = started(&engine, data.path());

    send(&mut app, "read @notes.txt");
    assert!(app.busy() && app.turn.run.is_none(), "the files are read off the screen's thread");
    send(&mut app, "and then this");
    assert!(prompts(&engine).is_empty(), "nothing went ahead");
    assert!(screen(&app).contains("and then this  (queued)"), "{}", screen(&app));

    until(&mut app, |app| app.turn.run.is_some() && !prompts(&engine).is_empty());
    let first = prompts(&engine);
    assert!(matches!(first.as_slice(), [prompt] if prompt.contains("notes.txt")), "{first:?}");
    until(&mut app, |app| !app.busy());
    assert_eq!(engine.steers(), ["and then this"], "the later one went to the run the first one started");
    let said: Vec<&str> = app
        .transcript
        .entries
        .iter()
        .filter_map(|entry| match entry {
            Entry::User(text) => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(said, ["read @notes.txt", "and then this"]);
}

#[test]
fn messages_to_a_run_arrive_in_the_order_typed_even_when_the_first_names_a_file() {
    let data = tempfile::tempdir().expect("data");
    std::fs::write(data.path().join("notes.txt"), "the notes").expect("write");
    let slow = vec![
        EngineEvent::SessionStarted(SessionId("s".into())),
        EngineEvent::TextDelta("Working".into()),
        EngineEvent::TextDelta(" on it.".into()),
        EngineEvent::Completed { duration_ms: 1, cost_usd: None },
    ];
    let engine =
        FakeEngine::new(vec![Model::new("keyed:scripted", "Scripted")], slow).with_delay(Duration::from_millis(150));
    let mut app = started(&engine, data.path());
    send(&mut app, "start");
    until(&mut app, |app| app.turn.run.is_some());

    send(&mut app, "see @notes.txt");
    send(&mut app, "plain words");
    assert!(engine.steers().is_empty(), "the plain one waits behind the one still reading its file");
    let waiting = screen(&app);
    assert!(waiting.contains("see @notes.txt  (queued)") && waiting.contains("plain words  (queued)"), "{waiting}");

    until(&mut app, |_| engine.steers().len() == 2);
    let steers = engine.steers();
    assert!(steers[0].contains("notes.txt"), "{steers:?}");
    assert_eq!(steers[1], "plain words");
}

#[test]
fn export_writes_the_file_in_the_background_and_says_so() {
    let data = tempfile::tempdir().expect("data");
    let engine = FakeEngine::new(vec![Model::new("keyed:scripted", "Scripted")], script());
    let mut app = started(&engine, data.path());
    send(&mut app, "hello");
    until(&mut app, |app| !app.busy() && app.transcript.entries.iter().any(|e| matches!(e, Entry::Assistant(_))));

    send(&mut app, "/export out.md");
    until(&mut app, |app| screen(app).contains("Wrote the conversation to"));
    let written = std::fs::read_to_string(data.path().join("out.md")).expect("the file");
    assert!(written.contains("> hello") && written.contains("Done."), "{written}");

    send(&mut app, "/export missing/folder/out.md");
    until(&mut app, |app| matches!(app.transcript.entries.last(), Some(Entry::Error(text)) if text.contains("out.md")));
}
