use agent_engine::model::scripted::ScriptedModel;
use agent_engine::model::{ModelError, Reply};
use agent_engine::transcript::{Block, Message, StopReason, Usage};
use domain::{ModelOption, PermissionMode};
use serde_json::json;

use crate::{Rig, tool_use};

fn measured(blocks: Vec<Block>, prompt: u64) -> Result<Reply, ModelError> {
    let stop = if blocks.iter().any(|b| matches!(b, Block::ToolUse { .. })) {
        StopReason::ToolUse
    } else {
        StopReason::EndTurn
    };
    Ok(Reply {
        message: Message::assistant(blocks),
        stop,
        usage: Usage { input: 1_000, output: 50, cache_read: prompt - 1_000, cache_write: 0 },
    })
}

#[test]
fn the_provider_s_own_token_count_decides_when_to_compact() {
    let mut rig = Rig::new(PermissionMode::Bypass);
    rig.workspace.model = ModelOption::new("m", "M").with_window(100_000);
    let model = ScriptedModel::new([
        measured(vec![tool_use("r", "Read", json!({"file_path": "notes.txt"}))], 95_000),
        ScriptedModel::reply(vec![Block::Text("<summary>read notes</summary>".into())]),
        ScriptedModel::reply(vec![Block::Text("Done.".into())]),
    ]);
    let mut history = vec![Message::user("read the notes"), Message::assistant(vec![]), Message::user("go")];
    rig.run(&model, &mut history).expect("run");
    let asked = model.asked();
    let summary = asked[1].messages.last().map(Message::text).unwrap_or_default();
    assert!(summary.contains("Write a detailed summary"), "the second call summarises: {summary}");
}
