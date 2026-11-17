use std::sync::{Arc, Mutex};

use agent_engine::Engine;
use agent_engine::model::scripted::ScriptedModel;
use agent_engine::sessions::InMemorySessions;
use agent_engine::transcript::Block;
use domain::{EngineEvent, PermissionMode, PromptRequest};
use serde_json::json;

use crate::{Source, finish, model, start};

fn engine(scripted: &Arc<ScriptedModel>) -> Engine {
    Engine::new(Arc::new(Source(Mutex::new(vec![Arc::clone(scripted)]))), Arc::new(InMemorySessions::default()))
}

#[test]
fn the_run_s_own_rules_turn_limit_and_appended_prompt_apply() {
    let dir = tempfile::tempdir().expect("dir");
    let scripted = Arc::new(ScriptedModel::new([
        ScriptedModel::reply(vec![Block::ToolUse {
            id: "b".into(),
            name: "Bash".into(),
            input: json!({"command": "touch made.txt"}),
        }]),
        ScriptedModel::reply(vec![Block::Text("never asked".into())]),
    ]));
    let mut request = PromptRequest::plain(dir.path().into(), "make it", model());
    request.policy.permission = PermissionMode::Bypass;
    request.policy.disallowed_tools = vec!["Bash(touch:*)".into()];
    request.session.append_system = Some("Answer in French.".into());
    request.policy.max_turns = Some(1);
    let events = finish(&start(&engine(&scripted), &request));

    assert!(!dir.path().join("made.txt").exists(), "--disallowedTools refused it even in bypass");
    assert!(scripted.asked()[0].system.contains("\n\nAnswer in French."), "appended to the system prompt");
    assert_eq!(scripted.asked().len(), 1, "one turn only");
    assert!(matches!(events.last(), Some(EngineEvent::Failed(_))), "{events:?}");
}

#[test]
fn the_run_s_ask_rule_asks_even_for_a_call_its_allow_rule_covers() {
    let dir = tempfile::tempdir().expect("dir");
    let scripted = Arc::new(ScriptedModel::new([
        ScriptedModel::reply(vec![Block::ToolUse {
            id: "b".into(),
            name: "Bash".into(),
            input: json!({"command": "touch made.txt"}),
        }]),
        ScriptedModel::reply(vec![Block::Text("done".into())]),
    ]));
    let mut request = PromptRequest::plain(dir.path().into(), "make it", model());
    request.policy.allowed_tools = vec!["Bash(touch:*)".into()];
    request.policy.ask_tools = vec!["Bash(touch:*)".into()];
    let (events, mut handle) = crate::start_with_handle(&engine(&scripted), &request);

    let asked = std::iter::from_fn(|| events.recv_timeout(std::time::Duration::from_secs(10)).ok())
        .any(|event| matches!(event, EngineEvent::Asked(_)));

    assert!(asked, "the ask rule wins over the allow rule");
    assert!(!dir.path().join("made.txt").exists());
    handle.cancel();
}

#[test]
fn a_malformed_rule_from_the_command_line_fails_the_run() {
    let dir = tempfile::tempdir().expect("dir");
    let scripted = Arc::new(ScriptedModel::new([]));
    let mut request = PromptRequest::plain(dir.path().into(), "go", model());
    request.policy.allowed_tools = vec!["Bash(git".into()];
    let events = finish(&start(&engine(&scripted), &request));
    assert!(
        matches!(events.last(), Some(EngineEvent::Failed(message)) if message.contains("--allowedTools")),
        "{events:?}"
    );
}
