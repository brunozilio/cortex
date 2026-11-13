use agent_engine::model::scripted::ScriptedModel;
use agent_engine::transcript::{Block, Message, Role};
use domain::{EngineEvent, PermissionMode};
use serde_json::json;

use crate::{Rig, tool_use};

fn text(said: &str) -> Result<agent_engine::model::Reply, agent_engine::model::ModelError> {
    ScriptedModel::reply(vec![Block::Text(said.into())])
}

#[test]
fn a_serious_finding_sends_the_agent_back_once_and_the_review_is_not_repeated() {
    let mut rig = Rig::new(PermissionMode::Bypass);
    rig.workspace.policy.auto_review = true;
    let finding = r#"{"findings": [{"severity": "high", "file": "made.txt", "line": 1, "issue": "says hi, the task asked for hello"}]}"#;
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![tool_use("w", "Write", json!({ "file_path": "made.txt", "content": "hi\n" }))]),
        text("Done."),
        text(finding),
        text("Fixed it."),
    ]);
    let mut history = vec![Message::user("write hello into made.txt")];

    rig.run(&model, &mut history).expect("run");

    let reviewed: Vec<_> = rig.events().into_iter().filter(|event| matches!(event, EngineEvent::Reviewed(_))).collect();
    assert_eq!(reviewed.len(), 1, "one review per run");
    let fed_back = history.iter().filter(|message| message.role == Role::User).map(Message::text);
    assert!(fed_back.into_iter().any(|said| said.starts_with(domain::review::MARK) && said.contains("made.txt:1")));
    assert_eq!(model.asked().len(), 4);
}

#[test]
fn a_run_that_changed_nothing_is_not_reviewed() {
    let mut rig = Rig::new(PermissionMode::Bypass);
    rig.workspace.policy.auto_review = true;
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![tool_use("r", "Read", json!({ "file_path": "notes.txt" }))]),
        text("It says alpha."),
    ]);

    rig.run(&model, &mut vec![Message::user("what does it say")]).expect("run");

    assert!(!rig.events().iter().any(|event| matches!(event, EngineEvent::Reviewed(_))));
    assert_eq!(model.asked().len(), 2);
}
