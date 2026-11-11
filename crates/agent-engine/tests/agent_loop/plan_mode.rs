use std::sync::{Arc, Mutex};

use agent_engine::control::Control;
use agent_engine::model::scripted::ScriptedModel;
use agent_engine::transcript::{Block, Message};
use domain::{AskKind, Decision, EngineEvent, PermissionMode};
use serde_json::json;

use crate::{Rig, tool_use};

fn answering(rig: &Rig, decision: Decision) -> std::thread::JoinHandle<()> {
    let control: Arc<Control> = Arc::clone(&rig.control);
    let events: Arc<Mutex<Vec<EngineEvent>>> = Arc::clone(&rig.events);
    std::thread::spawn(move || {
        for _ in 0..100 {
            let asks: Vec<_> = events
                .lock()
                .expect("events")
                .iter()
                .filter_map(|event| match event {
                    EngineEvent::Asked(ask) => Some(ask.id.clone()),
                    _ => None,
                })
                .collect();
            for id in asks {
                control.answer(&id, decision.clone());
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    })
}

fn results(history: &[Message]) -> Vec<(String, bool)> {
    history
        .iter()
        .flat_map(|m| &m.blocks)
        .filter_map(|block| match block {
            Block::ToolResult { content, is_error, .. } => Some((content.clone(), *is_error)),
            _ => None,
        })
        .collect()
}

fn write_after_plan() -> ScriptedModel {
    ScriptedModel::new([
        ScriptedModel::reply(vec![tool_use("p", "ExitPlanMode", json!({"plan": "## Plan\n1. Write out.txt"}))]),
        ScriptedModel::reply(vec![tool_use("w", "Write", json!({"file_path": "out.txt", "content": "done"}))]),
        ScriptedModel::reply(vec![Block::Text("Done.".into())]),
    ])
}

#[test]
fn an_approved_plan_switches_the_mode_and_the_agent_implements_it_in_the_same_run() {
    let rig = Rig::new(PermissionMode::Plan);
    let answerer = answering(&rig, Decision::Approve(PermissionMode::AcceptEdits));
    let mut history = vec![Message::user("plan it")];
    rig.run(&write_after_plan(), &mut history).expect("run");
    answerer.join().expect("answerer");

    let events = rig.events();
    assert!(events.iter().any(|event| matches!(
        event,
        EngineEvent::Asked(ask) if matches!(&ask.kind, AskKind::Plan { plan } if plan.contains("Write out.txt"))
    )));
    assert!(events.contains(&EngineEvent::ModeChanged(PermissionMode::AcceptEdits)));
    assert_eq!(rig.control.mode(), PermissionMode::AcceptEdits);
    assert_eq!(std::fs::read_to_string(rig.dir.path().join("out.txt")).expect("written"), "done");
    let results = results(&history);
    assert!(!results[0].1 && results[0].0.contains("approved"), "{results:?}");
}

#[test]
fn keeping_on_planning_leaves_the_agent_read_only() {
    let rig = Rig::new(PermissionMode::Plan);
    let answerer = answering(&rig, Decision::Deny);
    let mut history = vec![Message::user("plan it")];
    rig.run(&write_after_plan(), &mut history).expect("run");
    answerer.join().expect("answerer");

    assert_eq!(rig.control.mode(), PermissionMode::Plan);
    assert!(!rig.dir.path().join("out.txt").exists(), "nothing is written while planning");
    assert!(!rig.events().iter().any(|event| matches!(event, EngineEvent::ModeChanged(_))));
    let results = results(&history);
    assert!(results[0].1 && results[0].0.contains("keep planning"), "{results:?}");
}

#[test]
fn exit_plan_mode_outside_planning_asks_nothing() {
    let rig = Rig::new(PermissionMode::AcceptEdits);
    let mut history = vec![Message::user("go")];
    rig.run(&write_after_plan(), &mut history).expect("run");

    assert!(!rig.events().iter().any(|event| matches!(event, EngineEvent::Asked(_))));
    assert_eq!(rig.control.mode(), PermissionMode::AcceptEdits);
    assert!(results(&history)[0].1, "not in plan mode");
}

#[test]
fn entering_plan_mode_makes_the_agent_read_only() {
    let rig = Rig::new(PermissionMode::AcceptEdits);
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![tool_use("e", "EnterPlanMode", json!({}))]),
        ScriptedModel::reply(vec![tool_use("w", "Write", json!({"file_path": "out.txt", "content": "x"}))]),
        ScriptedModel::reply(vec![Block::Text("Planning.".into())]),
    ]);
    let mut history = vec![Message::user("this is big")];
    rig.run(&model, &mut history).expect("run");

    assert_eq!(rig.control.mode(), PermissionMode::Plan);
    assert!(rig.events().contains(&EngineEvent::ModeChanged(PermissionMode::Plan)));
    assert!(!rig.dir.path().join("out.txt").exists());
    let results = results(&history);
    assert!(!results[0].1, "{results:?}");
    assert!(results[1].1, "Write is refused once planning: {results:?}");
}
