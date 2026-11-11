//! Messages steered into a run, and taken back before the model saw them.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use agent_engine::NativeEngine;
use agent_engine::model::scripted::ScriptedModel;
use agent_engine::sessions::InMemorySessions;
use agent_engine::transcript::Block;
use domain::{Decision, EngineEvent, PermissionMode, PromptRequest};
use serde_json::json;

use crate::{Source, finish, model, start_with_handle};

#[test]
fn the_task_list_is_shown_whenever_the_agent_changes_it() {
    let dir = tempfile::tempdir().expect("dir");
    let scripted = Arc::new(ScriptedModel::new([
        ScriptedModel::reply(vec![Block::ToolUse {
            id: "t".into(),
            name: "TaskCreate".into(),
            input: json!({"subject": "Write the tests"}),
        }]),
        ScriptedModel::reply(vec![Block::ToolUse {
            id: "u".into(),
            name: "TaskUpdate".into(),
            input: json!({"task_id": "plan-1", "status": "in_progress"}),
        }]),
        ScriptedModel::reply(vec![Block::Text("Working.".into())]),
    ]));
    let engine = NativeEngine::new(
        Arc::new(Source(Mutex::new(vec![Arc::clone(&scripted)]))),
        Arc::new(InMemorySessions::default()),
    );
    let request = PromptRequest::plain(dir.path().into(), "plan it", model());
    let (events, _handle) = start_with_handle(&engine, &request);
    let lists: Vec<Vec<domain::events::TaskItem>> = finish(&events)
        .into_iter()
        .filter_map(|event| match event {
            EngineEvent::Tasks(tasks) => Some(tasks),
            _ => None,
        })
        .collect();
    let task = |status: &str| domain::events::TaskItem {
        id: "plan-1".into(),
        subject: "Write the tests".into(),
        status: status.into(),
    };
    assert_eq!(lists, [vec![task("pending")], vec![task("in_progress")]]);
}

#[test]
fn messages_taken_back_before_the_next_turn_never_reach_the_model() {
    let dir = tempfile::tempdir().expect("dir");
    let scripted = Arc::new(ScriptedModel::new([
        ScriptedModel::reply(vec![Block::ToolUse {
            id: "b".into(),
            name: "Bash".into(),
            input: json!({"command": "echo hi"}),
        }]),
        ScriptedModel::reply(vec![Block::Text("Skipped.".into())]),
    ]));
    let engine = NativeEngine::new(
        Arc::new(Source(Mutex::new(vec![Arc::clone(&scripted)]))),
        Arc::new(InMemorySessions::default()),
    );
    let mut request = PromptRequest::plain(dir.path().into(), "say hi", model());
    request.permission = PermissionMode::Ask;
    let (events, mut handle) = start_with_handle(&engine, &request);
    let ask = loop {
        match events.recv_timeout(Duration::from_secs(10)).expect("an ask") {
            EngineEvent::Asked(ask) => break ask,
            _ => continue,
        }
    };

    handle.steer("first".into());
    handle.steer("second".into());
    handle.unsteer();
    let unsteered = loop {
        match events.recv_timeout(Duration::from_secs(10)).expect("taken back") {
            EngineEvent::Unsteered(texts) => break texts,
            _ => continue,
        }
    };
    assert_eq!(unsteered, ["first", "second"]);

    handle.answer(&ask.id, &Decision::Deny);
    let rest = finish(&events);
    assert!(matches!(rest.last(), Some(EngineEvent::Completed { .. })), "{rest:?}");
    assert!(!rest.iter().any(|event| matches!(event, EngineEvent::Steered(_))), "{rest:?}");
    let next = scripted.asked()[1].messages.last().cloned().expect("results");
    assert!(!next.text().contains("first"), "{next:?}");
}
