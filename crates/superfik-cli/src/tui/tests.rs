//! The chat end to end on a scripted engine, drawn on a test terminal.

use std::sync::Arc;
use std::time::{Duration, Instant};

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use superfik_backend::{InMemorySecretStore, InMemoryStore, LocalServices, Memories};
use superfik_core::{AgentAsk, AskId, AskKind, EngineKind, SessionId, ToolCall};
use superfik_engine::EngineRegistry;
use superfik_engine::fake::FakeEngine;
use superfik_sdk::{Decision, EngineEvent, Model, PermissionMode, Superfik};

use super::app::App;
use super::{keys, view};

fn superfik(engine: &FakeEngine, data: &std::path::Path) -> Superfik {
    let mut services = LocalServices::with_engines(EngineRegistry::new().with(Arc::new(engine.clone())));
    services.memory = Memories::plain(Arc::new(InMemoryStore::default()));
    services.memory_tool = false;
    let secrets = Arc::new(InMemorySecretStore::default());
    services.plugins = services.plugins.with_secrets(secrets.clone());
    services.providers = services.providers.with_secrets(secrets);
    Superfik::builder().data_dir(data).services(services).build().expect("superfik")
}

fn until(app: &mut App, done: impl Fn(&App) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !done(app) {
        assert!(Instant::now() < deadline, "timed out");
        if let Ok(msg) = app.rx.recv_timeout(Duration::from_millis(50)) {
            app.message(msg);
        }
    }
}

fn screen(app: &App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).expect("terminal");
    terminal.draw(|frame| view::draw(frame, app)).expect("draw");
    let buffer = terminal.backend().buffer().clone();
    (0..buffer.area.height)
        .map(|y| (0..buffer.area.width).map(|x| buffer[(x, y)].symbol().to_owned()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

fn type_text(app: &mut App, text: &str) {
    for c in text.chars() {
        keys::key(app, KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
    }
}

fn press(app: &mut App, code: KeyCode) {
    keys::key(app, KeyEvent::new(code, KeyModifiers::NONE));
}

#[test]
fn a_prompt_streams_its_reply_tools_and_an_answered_question() {
    let data = tempfile::tempdir().expect("data");
    let ask = AgentAsk {
        id: AskId("a-1".into()),
        kind: AskKind::Permission { tool: "Bash".into(), subject: "cargo test".into(), rememberable: true },
    };
    let script = vec![
        EngineEvent::SessionStarted(SessionId("s".into())),
        EngineEvent::ToolStarted(ToolCall::new("t1", "Read", "src/main.rs")),
        EngineEvent::ToolFinished { id: "t1".into() },
        EngineEvent::Asked(ask),
        EngineEvent::TextDelta("All tests pass.".into()),
        EngineEvent::Completed { duration_ms: 5, cost_usd: Some(0.5) },
    ];
    let model = Model::new(EngineKind::Native, "anthropic:scripted", "Scripted");
    let engine = FakeEngine::new(EngineKind::Native, vec![model], script).with_delay(Duration::from_millis(60));
    let mut app = App::new(superfik(&engine, data.path()), data.path().into(), None, None);
    until(&mut app, |app| app.session.is_some());
    assert!(screen(&app).contains("Scripted · edits"), "{}", screen(&app));

    type_text(&mut app, "run the tests");
    press(&mut app, KeyCode::Enter);
    until(&mut app, |app| app.ask.is_some());
    let asking = screen(&app);
    assert!(asking.contains("Allow Bash: cargo test?"), "{asking}");
    assert!(asking.contains("● Read(src/main.rs)"), "{asking}");
    type_text(&mut app, "y");
    until(&mut app, |app| !app.busy());
    let done = screen(&app);
    assert!(done.contains("> run the tests") && done.contains("● All tests pass."), "{done}");
    assert!(done.contains("$0.5000"), "{done}");
    assert_eq!(engine.answers(), [(AskId("a-1".into()), Decision::Allow)]);
}

#[test]
fn shift_tab_and_commands_change_the_chat_without_the_agent() {
    let data = tempfile::tempdir().expect("data");
    let model = Model::new(EngineKind::Native, "anthropic:scripted", "Scripted");
    let engine = FakeEngine::new(EngineKind::Native, vec![model], Vec::new());
    let mut app = App::new(superfik(&engine, data.path()), data.path().into(), None, Some(PermissionMode::Ask));
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
    assert!(screen(&app).contains("/compact   Summarize"), "{}", screen(&app));
    assert!(engine.requests().is_empty(), "nothing reached the agent");
}
