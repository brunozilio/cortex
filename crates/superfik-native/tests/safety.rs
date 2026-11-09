//! The safety check of autonomous mode through the agent loop: an action
//! that edits or executes is graded first and held back above the line;
//! reading never waits for it, and supervised modes never call it.

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use serde_json::json;
use superfik_core::{Effort, EngineEvent, PermissionMode};
use superfik_native::agent::Agent;
use superfik_native::control::Control;
use superfik_native::guard::Safety;
use superfik_native::model::TurnRequest;
use superfik_native::model::scripted::ScriptedModel;
use superfik_native::tools::{self, Workspace};
use superfik_native::transcript::{Block, Message};

fn says(text: &str) -> Result<superfik_native::model::Reply, superfik_native::model::ModelError> {
    ScriptedModel::reply(vec![Block::Text(text.into())])
}

fn calls(
    name: &str,
    input: serde_json::Value,
) -> Result<superfik_native::model::Reply, superfik_native::model::ModelError> {
    ScriptedModel::reply(vec![Block::ToolUse { id: "t1".into(), name: name.into(), input }])
}

fn run(mode: PermissionMode, model: &ScriptedModel, dir: &std::path::Path) -> (String, Vec<TurnRequest>) {
    let mut workspace = Workspace::new(dir);
    workspace.safety = Some(Arc::new(Safety::default()));
    let control = Arc::new(Control::new(mode, Arc::new(AtomicBool::new(false))));
    let events: Arc<Mutex<Vec<EngineEvent>>> = Arc::default();
    let sink = Arc::clone(&events);
    let emit = move |event: EngineEvent| sink.lock().expect("events").push(event);
    let tools = tools::standard();
    let agent = Agent {
        api: model,
        tools: &tools,
        workspace: &workspace,
        control: &control,
        emit: &emit,
        request: TurnRequest {
            model: "m".into(),
            system: "s".into(),
            messages: Vec::new(),
            tools: Agent::specs(&tools),
            effort: Effort::Medium,
            max_tokens: 1000,
        },
        agents: &[],
        depth: 0,
        turn_limit: superfik_native::agent::MAX_TURNS,
    };
    let mut history = vec![Message::user("clean up")];
    agent.run(&mut history).expect("run");
    let result = history
        .iter()
        .flat_map(|m| &m.blocks)
        .find_map(|block| match block {
            Block::ToolResult { content, .. } => Some(content.clone()),
            _ => None,
        })
        .unwrap_or_default();
    (result, model.asked())
}

#[test]
fn a_harmful_command_is_held_back_in_autonomous_mode() {
    let dir = tempfile::tempdir().expect("dir");
    std::fs::write(dir.path().join("keep.txt"), "work").expect("write");
    let model = ScriptedModel::new([
        calls("Bash", json!({"command": "rm keep.txt"})),
        says("<severity>88</severity><category>Irreversible Local Destruction</category>"),
        says("I could not remove it."),
    ]);
    let (result, asked) = run(PermissionMode::Bypass, &model, dir.path());
    assert!(result.contains("Irreversible Local Destruction"), "{result}");
    assert!(dir.path().join("keep.txt").exists(), "the command never ran");
    assert!(asked[1].system.contains("security monitor"));
    assert!(asked[1].messages[1].text().contains("{\"user\":\"clean up\"}"));
}

#[test]
fn a_routine_command_runs_after_the_check() {
    let dir = tempfile::tempdir().expect("dir");
    let model = ScriptedModel::new([
        calls("Bash", json!({"command": "echo done"})),
        says("<severity>5</severity>"),
        says("Done."),
    ]);
    let (result, _) = run(PermissionMode::Bypass, &model, dir.path());
    assert!(result.contains("done"), "{result}");
}

#[test]
fn reading_and_supervised_modes_never_call_the_check() {
    let dir = tempfile::tempdir().expect("dir");
    std::fs::write(dir.path().join("a.txt"), "alpha").expect("write");
    let reading = ScriptedModel::new([calls("Read", json!({"file_path": "a.txt"})), says("It says alpha.")]);
    let (result, asked) = run(PermissionMode::Bypass, &reading, dir.path());
    assert!(result.contains("alpha"));
    assert_eq!(asked.len(), 2, "no check for a read");

    let editing =
        ScriptedModel::new([calls("Write", json!({"file_path": "b.txt", "content": "beta"})), says("Written.")]);
    let (_, asked) = run(PermissionMode::AcceptEdits, &editing, dir.path());
    assert_eq!(asked.len(), 2, "edits the user allowed are not checked");
    assert!(dir.path().join("b.txt").exists());
}
