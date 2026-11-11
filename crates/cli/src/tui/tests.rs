use std::sync::Arc;
use std::time::{Duration, Instant};

use agent_engine::fake::FakeEngine;
use backend::{InMemorySecretStore, LocalServices};
use domain::{AgentAsk, AskId, AskKind, ChoreAnswer, SessionCommand, SessionId, ToolCall};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use sdk::{Decision, EngineEvent, Model, PermissionMode, Sdk};

use super::app::App;
use super::{keys, view};

pub(super) fn sdk(engine: &FakeEngine, data: &std::path::Path) -> Sdk {
    let mut services = LocalServices::with_engine(Arc::new(engine.clone()));
    services.data = None;
    services.off.insert("memory_search".into());
    let secrets = Arc::new(InMemorySecretStore::default());
    services.providers = services.providers.with_secrets(secrets);
    Sdk::builder().data_dir(data).services(services).build().expect(brand::COMMAND)
}

pub(super) fn until(app: &mut App, done: impl Fn(&App) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !done(app) {
        assert!(Instant::now() < deadline, "timed out");
        if let Ok(msg) = app.rx.recv_timeout(Duration::from_millis(50)) {
            app.message(msg);
        }
    }
}

pub(super) fn screen(app: &App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).expect("terminal");
    terminal.draw(|frame| view::draw(frame, app)).expect("draw");
    let buffer = terminal.backend().buffer().clone();
    (0..buffer.area.height)
        .map(|y| (0..buffer.area.width).map(|x| buffer[(x, y)].symbol().to_owned()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn type_text(app: &mut App, text: &str) {
    for c in text.chars() {
        keys::key(app, KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
    }
}

pub(super) fn press(app: &mut App, code: KeyCode) {
    keys::key(app, KeyEvent::new(code, KeyModifiers::NONE));
}

#[test]
fn a_prompt_streams_its_reply_tools_and_an_answered_question() {
    let data = tempfile::tempdir().expect("data");
    let ask = AgentAsk {
        id: AskId("a-1".into()),
        kind: AskKind::Permission { tool: "Bash".into(), subject: "cargo test".into(), rememberable: true, rule: None },
    };
    let script = vec![
        EngineEvent::SessionStarted(SessionId("s".into())),
        EngineEvent::ThinkingDelta("I should read main first.".into()),
        EngineEvent::ToolStarted(ToolCall::new("t1", "Read", "src/main.rs")),
        EngineEvent::ToolFinished { id: "t1".into() },
        EngineEvent::Asked(ask),
        EngineEvent::TextDelta("All tests pass.".into()),
        EngineEvent::Completed { duration_ms: 5, cost_usd: Some(0.5) },
    ];
    let model = Model::new("keyed:scripted", "Scripted");
    let engine = FakeEngine::new(vec![model], script).with_delay(Duration::from_millis(60));
    let mut app = App::new(sdk(&engine, data.path()), data.path().into(), None, None);
    until(&mut app, |app| app.session.is_some());
    assert!(screen(&app).contains("Scripted · edits"), "{}", screen(&app));

    type_text(&mut app, "run the tests");
    press(&mut app, KeyCode::Enter);
    until(&mut app, |app| app.ask.is_some());
    let asking = screen(&app);
    assert!(asking.contains("Allow Bash: cargo test?"), "{asking}");
    assert!(asking.contains("● Read(src/main.rs)"), "{asking}");
    assert!(asking.contains("✻ Thought"), "the reasoning shows, folded once a tool runs: {asking}");
    assert!(asking.contains("Working…") && asking.contains("esc to interrupt"), "{asking}");
    type_text(&mut app, "y");
    until(&mut app, |app| !app.busy());
    let done = screen(&app);
    assert!(done.contains("> run the tests") && done.contains("● All tests pass."), "{done}");
    assert!(done.contains("$0.50"), "{done}");
    assert_eq!(engine.answers(), [(AskId("a-1".into()), Decision::Allow)]);
}

#[test]
fn shift_tab_and_commands_change_the_chat_without_the_agent() {
    let data = tempfile::tempdir().expect("data");
    let model = Model::new("keyed:scripted", "Scripted");
    let engine = FakeEngine::new(vec![model], Vec::new());
    let mut app = App::new(sdk(&engine, data.path()), data.path().into(), None, Some(PermissionMode::Ask));
    until(&mut app, |app| app.session.is_some());
    press(&mut app, KeyCode::BackTab);
    assert_eq!(app.mode(), PermissionMode::AcceptEdits);
    type_text(&mut app, "/mo");
    assert!(screen(&app).contains("/model"), "completions show");
    press(&mut app, KeyCode::Tab);
    assert_eq!(app.input.text(), "/model ");
    press(&mut app, KeyCode::Enter);
    assert!(screen(&app).contains("❯ Scripted"), "the model picker opens");
    press(&mut app, KeyCode::Esc);
    type_text(&mut app, "/help");
    press(&mut app, KeyCode::Enter);
    let help = match app.transcript.entries.last() {
        Some(super::transcript::Entry::Notice(text)) => text.clone(),
        other => panic!("no help: {other:?}"),
    };
    assert!(help.contains("/diff         Show") && help.contains("/verification Judge"), "{help}");
    assert!(screen(&app).contains("Ctrl+C quits"), "{}", screen(&app));
    assert!(engine.requests().is_empty(), "nothing reached the agent");
}

#[test]
#[ignore = "prints a screen to look at"]
fn preview() {
    let data = tempfile::tempdir().expect("data");
    let ask = AgentAsk {
        id: AskId("a".into()),
        kind: AskKind::Permission {
            tool: "Bash".into(),
            subject: "npm run dev".into(),
            rememberable: true,
            rule: None,
        },
    };
    let edit = EngineEvent::ToolResult {
        id: "t2".into(),
        output: "Edited src/main.rs · Added 2 lines, removed 1\n   12 -     let port = 3000;\n   12 +     let port = env_port();\n   13 +     log(port);\n   14       serve(port);".into(),
        failed: false,
    };
    let script = vec![
        EngineEvent::SessionStarted(SessionId("s".into())),
        EngineEvent::ThinkingDelta("The port is hardcoded; read it from the environment.".into()),
        EngineEvent::ToolStarted(ToolCall::new("t1", "Read", "src/main.rs")),
        EngineEvent::ToolFinished { id: "t1".into() },
        EngineEvent::TextDelta("I'll make the **port** configurable with `PORT`.".into()),
        EngineEvent::ToolStarted(ToolCall::new("t2", "Edit", "src/main.rs")),
        edit,
        EngineEvent::ToolFinished { id: "t2".into() },
        EngineEvent::Asked(ask),
    ];
    let model = Model::new("keyed:model-opus", "Opus 5.5");
    let engine = FakeEngine::new(vec![model], script).with_delay(Duration::from_millis(5));
    let mut app = App::new(sdk(&engine, data.path()), data.path().into(), None, None);
    until(&mut app, |app| app.session.is_some());
    type_text(&mut app, "make the port configurable");
    press(&mut app, KeyCode::Enter);
    until(&mut app, |app| app.ask.is_some());
    let mut terminal = Terminal::new(TestBackend::new(90, 40)).expect("terminal");
    terminal.draw(|frame| view::draw(frame, &app)).expect("draw");
    let buffer = terminal.backend().buffer().clone();
    for y in 0..buffer.area.height {
        let row: String = (0..buffer.area.width).map(|x| buffer[(x, y)].symbol().to_owned()).collect();
        println!("{}", row.trim_end());
    }
}

#[test]
fn a_dropped_image_is_attached_to_the_prompt_that_names_it() {
    let data = tempfile::tempdir().expect("data");
    let png = b"\x89PNG\r\n\x1a\n pixels";
    std::fs::write(data.path().join("shot one.png"), png).expect("png");
    let model = Model::new("keyed:scripted", "Scripted");
    let done = vec![EngineEvent::Completed { duration_ms: 1, cost_usd: None }];
    let engine = FakeEngine::new(vec![model], done);
    let mut app = App::new(sdk(&engine, data.path()), data.path().into(), None, None);
    until(&mut app, |app| app.session.is_some());
    type_text(&mut app, "what is");
    keys::paste(&mut app, "'shot one.png'");
    assert_eq!(app.input.text(), "what is [Image #1] ");
    keys::paste(&mut app, "not a file");
    press(&mut app, KeyCode::Enter);
    until(&mut app, |app| !app.busy() && !engine.requests().is_empty());
    let sent = engine.requests().remove(0);
    assert_eq!(sent.prompt, "what is [Image #1] not a file");
    assert_eq!(sent.images, [sdk::Image::from_bytes(png).expect("ok").expect("png")]);
}

#[test]
fn btw_answers_beside_the_conversation_and_esc_closes_it_without_interrupting() {
    let data = tempfile::tempdir().expect("data");
    let mut script = vec![
        EngineEvent::SessionStarted(SessionId("s".into())),
        EngineEvent::Chore(ChoreAnswer::Side("It edits main.rs.".into())),
    ];
    script.extend((0..6).map(|_| EngineEvent::TextDelta("…".into())));
    script.push(EngineEvent::TextDelta("All done.".into()));
    script.push(EngineEvent::Completed { duration_ms: 1, cost_usd: None });
    let model = Model::new("keyed:scripted", "Scripted");
    let engine = FakeEngine::new(vec![model], script).with_delay(Duration::from_millis(40));
    let mut app = App::new(sdk(&engine, data.path()), data.path().into(), None, None);
    until(&mut app, |app| app.session.is_some());

    type_text(&mut app, "/btw too early?");
    press(&mut app, KeyCode::Enter);
    assert!(screen(&app).contains("no conversation to ask about yet"), "{}", screen(&app));

    type_text(&mut app, "fix the bug");
    press(&mut app, KeyCode::Enter);
    until(&mut app, |app| app.session.as_ref().is_some_and(|s| s.id().is_some()));
    type_text(&mut app, "/btw which file?");
    press(&mut app, KeyCode::Enter);
    assert!(screen(&app).contains("which file?") && screen(&app).contains("answering…"), "{}", screen(&app));
    press(&mut app, KeyCode::Esc);
    assert!(app.side.is_none() && app.busy(), "Esc closed the answer, not the run");
    until(&mut app, |app| !app.busy());
    assert!(screen(&app).contains("All done."), "the run finished: {}", screen(&app));

    type_text(&mut app, "/btw which file?");
    press(&mut app, KeyCode::Enter);
    until(&mut app, |app| app.side.as_ref().is_some_and(|side| side.answer.is_some()));
    let shown = screen(&app);
    assert!(shown.contains("It edits main.rs."), "{shown}");
    press(&mut app, KeyCode::Esc);
    assert!(!screen(&app).contains("It edits main.rs."), "the answer is not kept in the conversation");
    assert_eq!(engine.requests().last().map(|r| r.command), Some(Some(SessionCommand::Side)));
}

#[test]
fn diffs_browses_the_changes_full_screen_by_file_and_scope() {
    let (data, repo) = (tempfile::tempdir().expect("data"), tempfile::tempdir().expect("repo"));
    let git = |args: &[&str]| {
        let done = std::process::Command::new("git").args(args).current_dir(repo.path()).output().expect("git");
        assert!(done.status.success(), "{args:?}");
    };
    git(&["init", "-q", "-b", "main"]);
    std::fs::write(repo.path().join("a.txt"), "one\n").expect("write");
    git(&["add", "a.txt"]);
    git(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "a"]);
    std::fs::write(repo.path().join("a.txt"), "two\n").expect("edit");
    std::fs::write(repo.path().join("new.txt"), "new\n").expect("new");
    let model = Model::new("keyed:scripted", "Scripted");
    let engine = FakeEngine::new(vec![model], Vec::new());
    let mut app = App::new(sdk(&engine, data.path()), repo.path().into(), None, None);
    until(&mut app, |app| app.session.is_some());
    let loaded = |app: &App| app.diffs.as_ref().is_some_and(|view| !view.loading());

    type_text(&mut app, "/diffs");
    press(&mut app, KeyCode::Enter);
    until(&mut app, loaded);
    let shown = screen(&app);
    assert!(shown.contains("Changes · uncommitted · 2 files"), "{shown}");
    assert!(shown.contains("M a.txt") && shown.contains("? new.txt"), "{shown}");
    assert!(shown.contains("1 - one") && shown.contains("1 + two"), "{shown}");
    press(&mut app, KeyCode::Down);
    assert!(screen(&app).contains("New file, not yet in git."), "{}", screen(&app));

    press(&mut app, KeyCode::Tab);
    press(&mut app, KeyCode::Tab);
    until(&mut app, loaded);
    assert!(screen(&app).contains("No staged changes."), "{}", screen(&app));
    press(&mut app, KeyCode::Char('q'));
    assert!(app.diffs.is_none() && screen(&app).contains(brand::NAME), "{}", screen(&app));
    assert!(engine.requests().is_empty(), "nothing reached the agent");
}

#[test]
fn ctrl_o_unfolds_a_long_output() {
    let data = tempfile::tempdir().expect("data");
    let model = Model::new("keyed:scripted", "Scripted");
    let engine = FakeEngine::new(vec![model], Vec::new());
    let mut app = App::new(sdk(&engine, data.path()), data.path().into(), None, None);
    until(&mut app, |app| app.session.is_some());
    let long = (1..=8).map(|n| format!("line {n}")).collect::<Vec<_>>().join("\n");
    app.transcript.apply(&EngineEvent::ToolStarted(ToolCall::new("t", "Bash", "seq 8")));
    app.transcript.apply(&EngineEvent::ToolResult { id: "t".into(), output: long, failed: false });
    assert!(screen(&app).contains("+5 lines (ctrl+o to expand)"), "{}", screen(&app));
    keys::key(&mut app, KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL));
    assert!(screen(&app).contains("line 8") && !screen(&app).contains("ctrl+o to expand"), "{}", screen(&app));
}
