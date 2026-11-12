use std::time::{Duration, Instant};

use agent_engine::fake::FakeEngine;
use domain::{AgentAsk, AskId, AskKind, ChoreAnswer, Rewound, SessionCommand, SessionId};
use ratatui::crossterm::event::KeyCode;
use sdk::{Decision, EngineEvent, Model};

use super::app::App;
use super::tests::{press, screen, sdk, type_text, until};
use super::transcript::Entry;

#[test]
fn a_finished_run_is_named_and_suggests_the_next_prompt_which_tab_takes() {
    let data = tempfile::tempdir().expect("data");
    let script = vec![
        EngineEvent::SessionStarted(SessionId("s".into())),
        EngineEvent::Chore(ChoreAnswer::Suggestion("run the tests".into())),
        EngineEvent::Completed { duration_ms: 1, cost_usd: None },
    ];
    let engine = FakeEngine::new(vec![Model::new("keyed:scripted", "Scripted")], script);
    let mut app = App::new(sdk(&engine, data.path()), data.path().into(), None, None);
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
    let engine = FakeEngine::new(vec![Model::new("keyed:scripted", "Scripted")], Vec::new());
    let mut app = App::new(sdk(&engine, data.path()), data.path().into(), None, None);
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

#[test]
fn the_sign_in_notices_go_once_connected_and_connected_goes_with_the_first_prompt() {
    let data = tempfile::tempdir().expect("data");
    let script = vec![EngineEvent::Completed { duration_ms: 1, cost_usd: None }];
    let engine = FakeEngine::new(vec![Model::new("keyed:scripted", "Scripted")], script);
    let mut app = App::new(sdk(&engine, data.path()), data.path().into(), None, None);
    until(&mut app, |app| app.session.is_some());
    app.transcript.passing("Signing in to Scripted…");
    app.message(super::app::Msg::Connected("Scripted".into()));
    let passing = |app: &App| app.transcript.entries.iter().filter(|e| matches!(e, Entry::Passing(_))).count();
    assert_eq!(passing(&app), 1);
    assert!(screen(&app).contains("Connected Scripted.") && !screen(&app).contains("Signing in"));

    type_text(&mut app, "hello");
    press(&mut app, KeyCode::Enter);
    assert_eq!(passing(&app), 0);
    assert!(!screen(&app).contains("Connected Scripted."), "{}", screen(&app));
}

#[test]
fn effort_without_a_level_opens_a_picker_at_the_current_one() {
    let data = tempfile::tempdir().expect("data");
    let engine = FakeEngine::new(vec![Model::new("keyed:scripted", "Scripted")], Vec::new());
    let mut app = App::new(sdk(&engine, data.path()), data.path().into(), None, None);
    until(&mut app, |app| app.session.is_some());
    type_text(&mut app, "/effort");
    press(&mut app, KeyCode::Enter);
    let picker = app.picker.as_ref().expect("a picker");
    assert_eq!(picker.items[picker.cursor].0, app.effort().cli_value());
    assert!(screen(&app).contains("xhigh"), "{}", screen(&app));
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Enter);
    assert!(app.picker.is_none());
    assert_eq!(app.effort(), sdk::Effort::High, "the level after medium");
}

fn asking_script(then: EngineEvent) -> Vec<EngineEvent> {
    let ask = AgentAsk { id: AskId("q".into()), kind: AskKind::Questions(Vec::new()) };
    vec![
        EngineEvent::Asked(ask),
        then,
        EngineEvent::TextDelta("ok".into()),
        EngineEvent::Completed { duration_ms: 1, cost_usd: None },
    ]
}

fn asked_app(script: Vec<EngineEvent>, data: &std::path::Path) -> App {
    let engine =
        FakeEngine::new(vec![Model::new("keyed:scripted", "Scripted")], script).with_delay(Duration::from_millis(60));
    let mut app = App::new(sdk(&engine, data), data.into(), None, None);
    until(&mut app, |app| app.session.is_some());
    type_text(&mut app, "ask me");
    press(&mut app, KeyCode::Enter);
    until(&mut app, |app| app.ask.is_some());
    assert!(app.started.is_some_and(|watch| watch.is_paused()), "the clock stops while the agent waits for an answer");
    app
}

#[test]
fn the_working_clock_stops_while_a_question_waits_and_runs_again_once_answered() {
    let data = tempfile::tempdir().expect("data");
    let mut app = asked_app(asking_script(EngineEvent::TextDelta("thanks".into())), data.path());
    app.answer(Decision::Deny);
    assert!(app.started.is_some_and(|watch| !watch.is_paused()));
}

#[test]
fn the_working_clock_runs_again_when_the_question_is_withdrawn() {
    let data = tempfile::tempdir().expect("data");
    let mut app = asked_app(asking_script(EngineEvent::AskWithdrawn(AskId("q".into()))), data.path());
    until(&mut app, |app| app.ask.is_none());
    assert!(app.started.is_some_and(|watch| !watch.is_paused()));
}

#[test]
fn a_waiting_question_shows_the_keys_that_answer_it() {
    let data = tempfile::tempdir().expect("data");
    let app = asked_app(asking_script(EngineEvent::TextDelta("thanks".into())), data.path());
    assert!(screen(&app).contains("space toggles a multiple choice · esc skips"), "{}", screen(&app));
}
