//! What `superfik -p` adds to a run: a turn limit, rules for this run only
//! and text after the system prompt.

use std::sync::{Arc, Mutex};

use serde_json::json;
use domain::{EngineEvent, PermissionMode, PromptRequest};
use agent_engine::NativeEngine;
use agent_engine::model::scripted::ScriptedModel;
use agent_engine::sessions::InMemorySessions;
use agent_engine::transcript::Block;

use crate::{Source, finish, model, start};

fn engine(scripted: &Arc<ScriptedModel>) -> NativeEngine {
    NativeEngine::new(Arc::new(Source(Mutex::new(vec![Arc::clone(scripted)]))), Arc::new(InMemorySessions::default()))
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
    request.permission = PermissionMode::Bypass;
    request.disallowed_tools = vec!["Bash(touch:*)".into()];
    request.append_system = Some("Answer in French.".into());
    request.max_turns = Some(1);
    let events = finish(&start(&engine(&scripted), &request));

    assert!(!dir.path().join("made.txt").exists(), "--disallowedTools refused it even in bypass");
    assert!(scripted.asked()[0].system.contains("\n\nAnswer in French."), "appended to the system prompt");
    assert_eq!(scripted.asked().len(), 1, "one turn only");
    assert!(matches!(events.last(), Some(EngineEvent::Failed(_))), "{events:?}");
}

#[test]
fn a_malformed_rule_from_the_command_line_fails_the_run() {
    let dir = tempfile::tempdir().expect("dir");
    let scripted = Arc::new(ScriptedModel::new([]));
    let mut request = PromptRequest::plain(dir.path().into(), "go", model());
    request.allowed_tools = vec!["Bash(git".into()];
    let events = finish(&start(&engine(&scripted), &request));
    assert!(
        matches!(events.last(), Some(EngineEvent::Failed(message)) if message.contains("--allowedTools")),
        "{events:?}"
    );
}
