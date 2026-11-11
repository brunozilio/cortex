use std::time::Duration;

use agent_engine::fake::FakeEngine;
use domain::{AgentAsk, AskId, AskKind, SessionId, ToolCall};
use ratatui::crossterm::event::KeyCode;
use sdk::{EngineEvent, Model, PermissionMode};

use super::app::App;
use super::tests::{press, screen, sdk, type_text, until};

fn script(after: Vec<EngineEvent>) -> Vec<EngineEvent> {
    let mut events = vec![
        EngineEvent::SessionStarted(SessionId("s".into())),
        EngineEvent::ToolStarted(ToolCall::new("p1", "ExitPlanMode", "")),
        EngineEvent::Asked(AgentAsk {
            id: AskId("p".into()),
            kind: AskKind::Plan { plan: "## Approach\n1. Write the landing page".into() },
        }),
    ];
    events.extend(after);
    events.push(EngineEvent::Completed { duration_ms: 5, cost_usd: None });
    events
}

fn planning(engine: &FakeEngine, data: &std::path::Path) -> App {
    let mut app = App::new(sdk(engine, data), data.into(), None, None);
    until(&mut app, |app| app.session.is_some());
    app.set_mode(PermissionMode::Plan);
    type_text(&mut app, "plan the site");
    press(&mut app, KeyCode::Enter);
    until(&mut app, |app| app.ask.is_some());
    app
}

#[test]
fn the_plan_shows_in_a_box_and_approving_it_switches_the_mode() {
    let data = tempfile::tempdir().expect("data");
    let engine = FakeEngine::new(
        vec![Model::new("keyed:scripted", "Scripted")],
        script(vec![
            EngineEvent::ModeChanged(PermissionMode::AcceptEdits),
            EngineEvent::ToolResult { id: "p1".into(), output: "approved".into(), failed: false },
            EngineEvent::ToolFinished { id: "p1".into() },
            EngineEvent::TextDelta("Writing it now.".into()),
        ]),
    )
    .with_delay(Duration::from_millis(60));
    let mut app = planning(&engine, data.path());

    let shown = screen(&app);
    for expected in
        ["Ready to code?", "╭", "Write the landing page", "1. Yes, and auto-accept edits", "No, keep planning"]
    {
        assert!(shown.contains(expected), "{expected}:\n{shown}");
    }
    assert!(!shown.contains("ExitPlanMode"), "the plan stands for the tool call:\n{shown}");

    press(&mut app, KeyCode::Enter);
    until(&mut app, |app| app.run.is_none());
    assert_eq!(app.mode(), PermissionMode::AcceptEdits);
    let shown = screen(&app);
    assert!(shown.contains("User approved the plan"), "{shown}");
    assert!(shown.contains("Scripted · edits"), "{shown}");
}

#[test]
fn keeping_on_planning_is_marked_on_the_plan() {
    let data = tempfile::tempdir().expect("data");
    let engine = FakeEngine::new(
        vec![Model::new("keyed:scripted", "Scripted")],
        script(vec![EngineEvent::ToolResult { id: "p1".into(), output: "keep planning".into(), failed: true }]),
    )
    .with_delay(Duration::from_millis(60));
    let mut app = planning(&engine, data.path());

    press(&mut app, KeyCode::Char('3'));
    until(&mut app, |app| app.run.is_none());
    assert_eq!(app.mode(), PermissionMode::Plan);
    assert!(screen(&app).contains("Kept planning"), "{}", screen(&app));
}

#[test]
fn the_chat_follows_the_agent_into_plan_mode() {
    let data = tempfile::tempdir().expect("data");
    let engine = FakeEngine::new(
        vec![Model::new("keyed:scripted", "Scripted")],
        vec![
            EngineEvent::SessionStarted(SessionId("s".into())),
            EngineEvent::ToolStarted(ToolCall::new("e1", "EnterPlanMode", "")),
            EngineEvent::ModeChanged(PermissionMode::Plan),
            EngineEvent::ToolFinished { id: "e1".into() },
            EngineEvent::Completed { duration_ms: 5, cost_usd: None },
        ],
    );
    let mut app = App::new(sdk(&engine, data.path()), data.path().into(), None, None);
    until(&mut app, |app| app.session.is_some());
    type_text(&mut app, "big change");
    press(&mut app, KeyCode::Enter);
    until(&mut app, |app| app.run.is_none());
    assert_eq!(app.mode(), PermissionMode::Plan);
    assert!(screen(&app).contains("Entered plan mode"), "{}", screen(&app));
}
