use std::sync::atomic::AtomicBool;

use domain::Effort;
use serde_json::json;

use super::*;
use crate::model::ToolSpec;
use crate::model::scripted::ScriptedModel;

fn last_turn() -> TurnRequest {
    TurnRequest {
        model: "m".into(),
        system: "base + reminder + rules".into(),
        messages: Vec::new(),
        tools: vec![ToolSpec { name: "Read".into(), description: "Reads".into(), schema: json!({}) }],
        effort: Effort::Low,
        max_tokens: 100,
    }
}

#[test]
fn the_summary_is_asked_with_the_last_turns_prefix_so_its_cache_is_read() {
    let model = ScriptedModel::new([ScriptedModel::reply(vec![Block::Text("<summary>done</summary>".into())])]);
    let mut history = vec![Message::user("a"), Message::user("b")];
    compact(&model, &last_turn(), &mut history, &AtomicBool::new(false)).expect("compact");
    let asked = &model.asked()[0];
    assert_eq!(asked.system, last_turn().system);
    assert_eq!(asked.tools, last_turn().tools);
    assert_eq!(asked.messages[..2], [Message::user("a"), Message::user("b")]);
}

#[test]
fn a_model_that_calls_a_tool_instead_is_asked_again_without_tools() {
    let call = Block::ToolUse { id: "t".into(), name: "Read".into(), input: json!({}) };
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![call]),
        ScriptedModel::reply(vec![Block::Text("<summary>done</summary>".into())]),
    ]);
    let mut history = vec![Message::user("a"), Message::user("b")];
    compact(&model, &last_turn(), &mut history, &AtomicBool::new(false)).expect("compact");
    assert!(model.asked()[1].tools.is_empty());
    assert!(history[0].text().ends_with("done"));
}
