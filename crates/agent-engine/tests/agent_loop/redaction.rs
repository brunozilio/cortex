use std::sync::Arc;

use agent_engine::model::scripted::ScriptedModel;
use agent_engine::transcript::{Block, Message};
use domain::{EngineEvent, PermissionMode};
use serde_json::json;

use crate::{Rig, tool_use};

const TOKEN: &str = "ghp_0123456789abcdefghijKLMN";
const DATABASE: &str = "postgres://app:hunter2hunter@db/main";

#[test]
fn secrets_in_a_command_s_output_reach_neither_the_model_nor_the_events() {
    let mut rig = Rig::new(PermissionMode::Bypass);
    std::fs::write(rig.dir.path().join(".env"), format!("DATABASE_URL={DATABASE}\n")).expect("env");
    rig.workspace.policy.redact = Some(Arc::new(agent_engine::tools::env_secrets(rig.dir.path())));
    let command = format!("echo token {TOKEN}; echo url {DATABASE}; echo plain");
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![tool_use("b", "Bash", json!({ "command": command }))]),
        ScriptedModel::reply(vec![Block::Text("Done".into())]),
    ]);
    let mut history = vec![Message::user("show")];
    rig.run(&model, &mut history).expect("run");

    let results: Vec<&str> = history
        .iter()
        .flat_map(|message| &message.blocks)
        .filter_map(|block| match block {
            Block::ToolResult { content, .. } => Some(content.as_str()),
            _ => None,
        })
        .collect();
    let sent = serde_json::to_string(&results).expect("results");
    let shown = serde_json::to_string(&rig.events()).expect("events");
    for leaked in [TOKEN, DATABASE] {
        assert!(!sent.contains(leaked), "the model saw {leaked}");
        assert!(!shown.contains(leaked), "the events carried {leaked}");
    }
    assert!(sent.contains("token [redacted]\\nurl [redacted]\\nplain"), "{sent}");
    let started = rig.events().into_iter().find_map(|event| match event {
        EngineEvent::ToolStarted(call) => call.input,
        _ => None,
    });
    assert!(started.is_some_and(|input| input["command"].as_str().is_some_and(|text| text.contains("[redacted]"))));
}

#[test]
fn without_the_flag_output_is_left_as_it_is() {
    let rig = Rig::new(PermissionMode::Bypass);
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![tool_use("b", "Bash", json!({ "command": format!("echo {TOKEN}") }))]),
        ScriptedModel::reply(vec![Block::Text("Done".into())]),
    ]);
    let mut history = vec![Message::user("show")];
    rig.run(&model, &mut history).expect("run");

    assert!(serde_json::to_string(&history).expect("history").contains(TOKEN));
}

#[test]
fn a_command_s_lines_show_while_it_runs_and_keep_secrets_out() {
    let mut rig = Rig::new(PermissionMode::Bypass);
    std::fs::write(rig.dir.path().join(".env"), format!("DATABASE_URL={DATABASE}\n")).expect("env");
    rig.workspace.policy.redact = Some(Arc::new(agent_engine::tools::env_secrets(rig.dir.path())));
    let command = format!("echo compiling one; sleep 0.5; echo url {DATABASE}; sleep 0.5; echo finished");
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![tool_use("b", "Bash", json!({ "command": command }))]),
        ScriptedModel::reply(vec![Block::Text("Done".into())]),
    ]);
    let mut history = vec![Message::user("build")];
    rig.run(&model, &mut history).expect("run");

    let lines: Vec<String> = rig
        .events()
        .into_iter()
        .filter_map(|event| match event {
            EngineEvent::ToolProgress { id, line } if id == "b" => Some(line),
            _ => None,
        })
        .collect();
    assert!(lines.iter().any(|line| line == "compiling one"), "{lines:?}");
    assert!(lines.iter().all(|line| !line.contains(DATABASE)), "{lines:?}");
}
