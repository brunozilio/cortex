use agent_engine::model::scripted::ScriptedModel;
use agent_engine::transcript::{Block, Message};
use domain::{EngineEvent, PermissionMode};
use serde_json::json;

use crate::{Rig, tool_use};

fn failing(times: usize) -> ScriptedModel {
    let mut replies: Vec<_> = (0..times)
        .map(|at| ScriptedModel::reply(vec![tool_use(&format!("b{at}"), "Bash", json!({ "command": "exit 1" }))]))
        .collect();
    replies.push(ScriptedModel::reply(vec![Block::Text("Done".into())]));
    ScriptedModel::new(replies)
}

#[test]
fn the_same_command_failing_three_times_moves_the_next_turns_to_the_escalation_model() {
    let mut rig = Rig::new(PermissionMode::Bypass);
    rig.workspace.policy.escalation = true;
    rig.workspace.models.escalation = Some("big-model".into());
    let model = failing(3);
    let mut history = vec![Message::user("fix it")];

    rig.run(&model, &mut history).expect("run");

    let models: Vec<String> = model.asked().into_iter().map(|request| request.model).collect();
    assert_eq!(models, ["m", "m", "m", "big-model"]);
    assert!(rig.events().contains(&EngineEvent::FellBack("big-model".into())));
    let nudged = history.iter().flat_map(|message| &message.blocks).any(
        |block| matches!(block, Block::ToolResult { content, .. } if content.contains("several times without success")),
    );
    assert!(nudged, "the agent is told why the model changed");
}

#[test]
fn without_the_flag_the_model_stays() {
    let mut rig = Rig::new(PermissionMode::Bypass);
    rig.workspace.models.escalation = Some("big-model".into());
    let model = failing(3);

    rig.run(&model, &mut vec![Message::user("fix it")]).expect("run");

    assert!(model.asked().iter().all(|request| request.model == "m"));
}
