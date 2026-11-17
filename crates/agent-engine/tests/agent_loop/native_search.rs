use agent_engine::model::Reply;
use agent_engine::model::scripted::ScriptedModel;
use agent_engine::transcript::{Block, Message, ReasoningOrigin, StopReason, Usage};
use domain::PermissionMode;
use serde_json::json;

use crate::Rig;

#[test]
fn a_paused_search_turn_goes_on_with_its_results_and_no_new_prompt() {
    let rig = Rig::new(PermissionMode::Bypass);
    let searched = Block::Native {
        origin: ReasoningOrigin::of("messages"),
        item: json!({"type": "web_search_tool_result", "tool_use_id": "s1", "content": []}),
    };
    let model = ScriptedModel::new([
        Ok(Reply {
            message: Message::assistant(vec![searched.clone()]),
            stop: StopReason::PauseTurn,
            usage: Usage::default(),
            served: None,
        }),
        ScriptedModel::reply(vec![Block::Text("Rust 2026 shipped.".into())]),
    ]);
    let mut history = vec![Message::user("what's new in rust?")];
    rig.run(&model, &mut history).expect("run");
    let asked = model.asked();
    assert_eq!(asked.len(), 2);
    let resent = &asked[1].messages;
    assert_eq!(resent.len(), 2, "no prompt is added to resume");
    assert_eq!(resent[1].blocks, vec![searched]);
    assert_eq!(history.last().map(Message::text).as_deref(), Some("Rust 2026 shipped."));
}
