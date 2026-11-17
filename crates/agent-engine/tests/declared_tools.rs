#![allow(clippy::expect_used, reason = "a failed setup step stops the test")]

use std::sync::{Arc, Mutex};

use agent_engine::agent::Agent;
use agent_engine::control::Control;
use agent_engine::model::TurnRequest;
use agent_engine::model::scripted::ScriptedModel;
use agent_engine::tools::{self, Workspace};
use agent_engine::transcript::{Block, Message};
use domain::{Effort, EngineEvent, PermissionMode};
use serde_json::json;

const WRAPPED_READ: &str = r#"
export const spec = {
  name: "Read",
  access: "read",
  input: { type: "object", properties: { file_path: { type: "string" } }, required: ["file_path"] },
  description: "Reads a file, shouting.",
};

export function execute(input, ctx) {
  ctx.log(`reading ${input.file_path}`);
  return ctx.builtin({ ...input, file_path: "notes.txt" }).toUpperCase();
}
"#;

fn run(dir: &std::path::Path, calls: Vec<Block>) -> (Vec<Message>, Vec<EngineEvent>) {
    let workspace = Workspace::new(dir);
    let control = Control::new(PermissionMode::Bypass, Arc::default());
    let tools = tools::standard();
    let events: Arc<Mutex<Vec<EngineEvent>>> = Arc::default();
    let seen = Arc::clone(&events);
    let emit = move |event: EngineEvent| seen.lock().expect("events").push(event);
    let model =
        ScriptedModel::new([ScriptedModel::reply(calls), ScriptedModel::reply(vec![Block::Text("Ok.".into())])]);
    let agent = Agent {
        api: &model,
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
        turn_limit: agent_engine::agent::MAX_TURNS,
    };
    let mut history = vec![Message::user("read")];
    agent.run(&mut history).expect("run");
    let events = events.lock().expect("events").clone();
    (history, events)
}

fn result_of(history: &[Message], id: &str) -> (String, bool) {
    history
        .iter()
        .flat_map(|message| message.blocks.iter())
        .find_map(|block| match block {
            Block::ToolResult { id: found, content, is_error } if found == id => Some((content.clone(), *is_error)),
            _ => None,
        })
        .expect("a result")
}

const IN_CHILD: &str = "DECLARED_TOOLS_CHILD";
const TEST: &str = "a_user_s_copy_of_a_built_in_tool_runs_its_execute_around_the_built_in";

#[test]
fn a_user_s_copy_of_a_built_in_tool_runs_its_execute_around_the_built_in() {
    if std::env::var_os(IN_CHILD).is_none() {
        let home = tempfile::tempdir().expect("home");
        let tools_folder = home.path().join(brand::DIR).join("tools");
        std::fs::create_dir_all(&tools_folder).expect("tools");
        std::fs::write(tools_folder.join("Read.js"), WRAPPED_READ).expect("read tool");
        let output = std::process::Command::new(std::env::current_exe().expect("test binary"))
            .args(["--exact", TEST, "--nocapture"])
            .env(IN_CHILD, "1")
            .env("HOME", home.path())
            .env("USERPROFILE", home.path())
            .output()
            .expect("child");
        let shown = format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
        assert!(output.status.success() && shown.contains("1 passed"), "{shown}");
        return;
    }

    let dir = tempfile::tempdir().expect("dir");
    std::fs::write(dir.path().join("notes.txt"), "alpha\nbeta\n").expect("write");
    let specs = Agent::specs(&tools::standard());
    let read = specs.iter().find(|spec| spec.name == "Read").expect("Read");
    assert_eq!(read.description, "Reads a file, shouting.");
    assert_eq!(read.schema["required"], json!(["file_path"]));

    let call = Block::ToolUse { id: "r".into(), name: "Read".into(), input: json!({ "file_path": "elsewhere.txt" }) };
    let glob = Block::ToolUse { id: "g".into(), name: "Glob".into(), input: json!({ "pattern": "*.txt" }) };
    let (history, events) = run(dir.path(), vec![call, glob]);

    let (read, failed) = result_of(&history, "r");
    assert!(!failed, "{read}");
    assert!(read.contains("ALPHA") && read.contains("BETA"), "{read}");
    assert!(
        events.contains(&EngineEvent::ToolLog { id: "r".into(), text: "reading elsewhere.txt".into() }),
        "{events:?}"
    );
    let (glob, failed) = result_of(&history, "g");
    assert!(!failed && glob.contains("notes.txt"), "the built-in execute passes through: {glob}");
}
