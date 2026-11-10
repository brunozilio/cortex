//! The chat around its runs: the side calls it makes by itself, and going
//! back to an earlier prompt.

use std::time::{Duration, Instant};

use ratatui::crossterm::event::KeyCode;
use superfik_core::{ChoreAnswer, Rewound, SessionCommand, SessionId};
use superfik_native::fake::FakeEngine;
use superfik_sdk::{EngineEvent, Model};

use super::app::App;
use super::tests::{press, screen, superfik, type_text, until};
use super::transcript::Entry;

#[test]
fn a_finished_run_is_named_and_suggests_the_next_prompt_which_tab_takes() {
    let data = tempfile::tempdir().expect("data");
    let script = vec![
        EngineEvent::SessionStarted(SessionId("s".into())),
        EngineEvent::Chore(ChoreAnswer::Suggestion("run the tests".into())),
        EngineEvent::Completed { duration_ms: 1, cost_usd: None },
    ];
    let engine = FakeEngine::new(vec![Model::new("anthropic:scripted", "Scripted")], script);
    let mut app = App::new(superfik(&engine, data.path()), data.path().into(), None, None);
    until(&mut app, |app| app.session.is_some());

    type_text(&mut app, "fix the login page");
    press(&mut app, KeyCode::Enter);
    until(&mut app, |app| app.ghost.is_some());
    assert!(screen(&app).contains("run the tests  (tab)"), "{}", screen(&app));
    press(&mut app, KeyCode::Tab);
    assert_eq!(app.input.text(), "run the tests");
    assert!(app.ghost.is_none());

    let deadline = Instant::now() + Duration::from_secs(5);
    let asked = || engine.requests().iter().filter_map(|r| r.command).collect::<Vec<_>>();
    while asked().len() < 3 {
        assert!(Instant::now() < deadline, "{:?}", asked());
        std::thread::sleep(Duration::from_millis(10));
    }
    let mut asked = asked();
    asked.sort_by_key(|c| format!("{c:?}"));
    assert_eq!(asked, [SessionCommand::HeadsUp, SessionCommand::Suggest, SessionCommand::Title]);
    let title = engine.requests().into_iter().find(|r| r.command == Some(SessionCommand::Title)).expect("title");
    assert_eq!(title.prompt, "fix the login page", "named for what was first asked");
}

#[test]
fn a_rewind_cuts_the_chat_before_its_prompt_and_gives_the_prompt_back() {
    let data = tempfile::tempdir().expect("data");
    let engine = FakeEngine::new(vec![Model::new("anthropic:scripted", "Scripted")], Vec::new());
    let mut app = App::new(superfik(&engine, data.path()), data.path().into(), None, None);
    app.transcript.push(Entry::User("write the notes".into()));
    app.checkpointed(0);
    app.transcript.notice("Written.");
    app.transcript.push(Entry::User("add more".into()));
    app.checkpointed(1);
    app.transcript.notice("Added.");

    type_text(&mut app, "/rewind");
    press(&mut app, KeyCode::Enter);
    let picker = app.picker.as_ref().expect("picker");
    assert_eq!(picker.items[0].0, "add more", "newest first");

    let restored = vec![data.path().join("more.txt")];
    app.rewound(Rewound { checkpoint: 1, prompt: "add more".into(), restored, conversation: true });
    let shown = screen(&app);
    assert!(shown.contains("Written.") && !shown.contains("Added."), "{shown}");
    assert!(shown.contains("Rewound: 1 file restored"), "{shown}");
    assert_eq!(app.input.text(), "add more", "the prompt is back to edit");
    assert_eq!(app.checkpoints.iter().map(|c| c.index).collect::<Vec<_>>(), [0]);
}
