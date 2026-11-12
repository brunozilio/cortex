use agent_engine::model::scripted::ScriptedModel;
use agent_engine::transcript::{Block, Message};
use domain::PermissionMode;
use serde_json::json;

use crate::{Rig, tool_use};

fn read(id: &str) -> Result<agent_engine::model::Reply, agent_engine::model::ModelError> {
    ScriptedModel::reply(vec![tool_use(id, "Read", json!({"file_path": "notes.txt"}))])
}

#[test]
fn a_third_identical_call_is_not_run_and_says_how_to_get_another_tool() {
    let rig = Rig::new(PermissionMode::Bypass);
    let model = ScriptedModel::new([
        read("r1"),
        read("r2"),
        read("r3"),
        ScriptedModel::reply(vec![Block::Text("Done".into())]),
    ]);
    let mut history = vec![Message::user("look")];
    rig.run(&model, &mut history).expect("run");
    let results: Vec<(bool, String)> = history
        .iter()
        .flat_map(|message| &message.blocks)
        .filter_map(|block| match block {
            Block::ToolResult { content, is_error, .. } => Some((*is_error, content.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(results.len(), 3);
    assert!(results[0].1.contains("alpha") && results[..2].iter().all(|(failed, _)| !failed), "{results:?}");
    assert!(results[2].0 && results[2].1.starts_with("Not run") && results[2].1.contains("select:"), "{results:?}");
}
