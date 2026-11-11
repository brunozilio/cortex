use agent_engine::agent::Ending;
use agent_engine::hooks::Hooks;
use agent_engine::model::ModelError;
use agent_engine::model::scripted::ScriptedModel;
use agent_engine::transcript::{Block, Message, Role};
use domain::interception::MAX_REJECTIONS;
use domain::{Effort, EngineEvent, PermissionMode};
use serde_json::json;

use crate::{Rig, tool_use};

fn answering(event: &str, output: serde_json::Value) -> Hooks {
    let command = format!("cat > {event}.json; printf '%s' '{output}'");
    Hooks::parse(&json!({event: [{"hooks": [{"type": "command", "command": command}]}]})).expect("hooks")
}

fn script(rig: &Rig, source: &str) -> Hooks {
    let folder = rig.dir.path().join("hooks");
    std::fs::create_dir_all(&folder).expect("hooks");
    std::fs::write(folder.join("hook.js"), source).expect("hook");
    Hooks::from_scripts(&folder)
}

fn input(rig: &Rig, event: &str) -> serde_json::Value {
    let text = std::fs::read_to_string(rig.dir.path().join(format!("{event}.json"))).expect("input");
    serde_json::from_str(&text).expect("json")
}

fn shown_text(rig: &Rig) -> String {
    rig.events()
        .into_iter()
        .filter_map(|event| match event {
            EngineEvent::TextDelta(text) => Some(text),
            _ => None,
        })
        .collect()
}

#[cfg(unix)]
#[test]
fn a_model_request_hook_adds_to_the_system_prompt_and_sets_the_effort() {
    let mut rig = Rig::new(PermissionMode::Bypass);
    let output = json!({"hookSpecificOutput": {"additionalContext": "Answer in Portuguese.", "effort": "low"}});
    rig.workspace.hooks = answering("ModelRequest", output);
    let model = ScriptedModel::new([ScriptedModel::reply(vec![Block::Text("Feito.".into())])]);
    rig.run(&model, &mut vec![Message::user("hi")]).expect("run");
    let asked = &model.asked()[0];
    assert!(asked.system.ends_with("Answer in Portuguese."), "{}", asked.system);
    assert_eq!(asked.effort, Effort::Low);
    let input = input(&rig, "ModelRequest");
    assert!(input["effort"].is_string() && input["maxTokens"].is_number(), "{input}");
}

#[cfg(unix)]
#[test]
fn a_model_request_hook_may_block_the_call() {
    let mut rig = Rig::new(PermissionMode::Bypass);
    rig.workspace.hooks = answering("ModelRequest", json!({"decision": "block", "reason": "no calls after 6pm"}));
    let model = ScriptedModel::new([]);
    let ending = rig.run(&model, &mut vec![Message::user("hi")]);
    assert!(
        matches!(&ending, Err(Ending::Failed(ModelError::Provider(why))) if why.contains("no calls after 6pm")),
        "{ending:?}"
    );
    assert!(model.asked().is_empty());
}

#[cfg(unix)]
#[test]
fn a_model_response_hook_sees_the_reply_and_rewrites_its_text_before_it_is_shown() {
    let mut rig = Rig::new(PermissionMode::Bypass);
    rig.workspace.hooks = answering("ModelResponse", json!({"hookSpecificOutput": {"text": "The key is [redacted]."}}));
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![
            Block::Text("The key is sk-123.".into()),
            tool_use("r", "Read", json!({"file_path": "notes.txt"})),
        ]),
        ScriptedModel::reply(vec![Block::Text("Done.".into())]),
    ]);
    let mut history = vec![Message::user("hi")];
    rig.run(&model, &mut history).expect("run");
    let first = history.iter().find(|message| message.role == Role::Assistant).expect("reply");
    assert_eq!(first.text(), "The key is [redacted].");
    assert_eq!(first.tool_uses().count(), 1, "tool calls are kept");
    assert!(!shown_text(&rig).contains("sk-123"), "{}", shown_text(&rig));
    assert!(shown_text(&rig).contains("[redacted]"));
    let input = input(&rig, "ModelResponse");
    assert_eq!(input["text"], "Done.");
    assert_eq!(input["stopReason"], "end_turn");
    assert!(input["toolCalls"].is_array());
}

#[test]
fn a_model_response_hook_may_reject_a_reply_so_the_model_answers_again() {
    let mut rig = Rig::new(PermissionMode::Bypass);
    rig.workspace.hooks = script(
        &rig,
        r#"export const event = "ModelResponse";
export default (input) => input.rejections === 0 ? { decision: "block", reason: "cite a file" } : undefined;"#,
    );
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![Block::Text("Trust me.".into())]),
        ScriptedModel::reply(vec![Block::Text("See src/a.rs.".into())]),
    ]);
    let mut history = vec![Message::user("hi")];
    rig.run(&model, &mut history).expect("run");
    let asked = model.asked();
    assert_eq!(asked.len(), 2);
    assert!(asked[1].system.contains("cite a file"), "the model hears why");
    assert!(!asked[0].system.contains("cite a file"));
    let replies: Vec<String> =
        history.iter().filter(|message| message.role == Role::Assistant).map(Message::text).collect();
    assert_eq!(replies, ["See src/a.rs."], "the rejected reply is dropped");
    assert!(!shown_text(&rig).contains("Trust me."));
}

#[test]
fn a_reply_rejected_too_often_ends_the_run() {
    let mut rig = Rig::new(PermissionMode::Bypass);
    rig.workspace.hooks = script(
        &rig,
        r#"export const event = "ModelResponse";
export default () => ({ decision: "block", reason: "never good" });"#,
    );
    let replies = (0..=MAX_REJECTIONS).map(|_| ScriptedModel::reply(vec![Block::Text("Try.".into())]));
    let model = ScriptedModel::new(replies.collect::<Vec<_>>());
    let ending = rig.run(&model, &mut vec![Message::user("hi")]);
    assert!(
        matches!(&ending, Err(Ending::Failed(ModelError::Provider(why))) if why.contains("never good")),
        "{ending:?}"
    );
    assert_eq!(model.asked().len(), MAX_REJECTIONS + 1);
}

#[test]
fn without_a_response_hook_text_streams_as_it_comes() {
    let rig = Rig::new(PermissionMode::Bypass);
    let model = ScriptedModel::new([ScriptedModel::reply(vec![Block::Text("Hello.".into())])]);
    rig.run(&model, &mut vec![Message::user("hi")]).expect("run");
    assert_eq!(shown_text(&rig), "Hello.");
}
