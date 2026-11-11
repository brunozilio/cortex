use agent_engine::model::scripted::ScriptedModel;
use agent_engine::transcript::{Block, Message, Role};
use domain::{EngineEvent, PermissionMode};
use serde_json::json;

use crate::{Rig, tool_use};

fn texts(message: &Message) -> Vec<String> {
    message
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Text(text) => Some(text.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_message_sent_while_a_tool_runs_reaches_the_model_with_its_results() {
    let rig = Rig::new(PermissionMode::AcceptEdits);
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![tool_use("t1", "Read", json!({"file_path": "notes.txt"}))]),
        ScriptedModel::reply(vec![Block::Text("Ok, in Portuguese.".into())]),
    ]);
    rig.control.steer("answer in Portuguese".into());
    rig.run(&model, &mut vec![Message::user("read the notes")]).expect("run");

    let asked = model.asked();
    assert_eq!(asked.len(), 2, "one run, two turns");
    let last = asked[1].messages.last().expect("a message");
    assert_eq!(last.role, Role::User);
    assert!(matches!(last.blocks.first(), Some(Block::ToolResult { id, .. }) if id == "t1"));
    assert!(texts(last).iter().any(|text| text.contains("answer in Portuguese")), "{last:?}");
    assert!(rig.events().contains(&EngineEvent::Steered("answer in Portuguese".into())));
}

#[test]
fn a_message_that_arrives_as_the_model_finishes_starts_another_turn() {
    let rig = Rig::new(PermissionMode::AcceptEdits);
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![Block::Text("Done.".into())]),
        ScriptedModel::reply(vec![Block::Text("Also done.".into())]),
    ]);
    rig.control.steer("and the tests too".into());
    rig.run(&model, &mut vec![Message::user("fix it")]).expect("run");

    let asked = model.asked();
    assert_eq!(asked.len(), 2, "the run went on for the message");
    let last = asked[1].messages.last().expect("a message");
    assert_eq!(last.role, Role::User);
    assert!(texts(last).iter().any(|text| text.contains("and the tests too")), "{last:?}");
}

#[test]
fn nothing_steered_ends_the_run_as_before() {
    let rig = Rig::new(PermissionMode::AcceptEdits);
    let model = ScriptedModel::new([ScriptedModel::reply(vec![Block::Text("Done.".into())])]);
    rig.run(&model, &mut vec![Message::user("fix it")]).expect("run");
    assert_eq!(model.asked().len(), 1);
    assert!(!rig.events().iter().any(|event| matches!(event, EngineEvent::Steered(_))));
}
