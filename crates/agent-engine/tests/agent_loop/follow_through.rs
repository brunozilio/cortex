use agent_engine::model::scripted::ScriptedModel;
use agent_engine::transcript::{Block, Message};
use domain::PermissionMode;
use serde_json::json;

use crate::{Rig, tool_use};

fn promised_then_finished() -> ScriptedModel {
    ScriptedModel::new([
        ScriptedModel::reply(vec![tool_use("b1", "Bash", json!({"command": "ls"}))]),
        ScriptedModel::reply(vec![Block::Text("Listed the files. Now I'll run the tests.".into())]),
        ScriptedModel::reply(vec![Block::Text(r#"{"verdict":"continue","nudge":"Run the tests."}"#.into())]),
        ScriptedModel::reply(vec![Block::Text("Ran them: all pass.".into())]),
    ])
}

#[test]
fn an_answer_that_promises_more_work_is_sent_back_to_do_it() {
    let mut rig = Rig::new(PermissionMode::Bypass);
    rig.workspace.policy.follow_through = true;
    let model = promised_then_finished();
    let mut history = vec![Message::user("list and test")];
    rig.run(&model, &mut history).expect("run");

    let judge = &model.asked()[2];
    assert!(judge.tools.is_empty(), "the judge only reads");
    assert!(judge.messages[0].text().contains("Now I'll run the tests."));
    assert!(history.iter().any(|message| message.text().starts_with(domain::follow_through::MARK)));
    assert_eq!(history.last().map(Message::text).as_deref(), Some("Ran them: all pass."));
}

#[test]
fn without_the_flag_the_answer_ends_the_run() {
    let rig = Rig::new(PermissionMode::Bypass);
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![tool_use("b1", "Bash", json!({"command": "ls"}))]),
        ScriptedModel::reply(vec![Block::Text("Now I'll run the tests.".into())]),
    ]);
    let mut history = vec![Message::user("list and test")];
    rig.run(&model, &mut history).expect("run");
    assert_eq!(model.asked().len(), 2);
}
