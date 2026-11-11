use std::sync::{Arc, Mutex};

use agent_engine::NativeEngine;
use agent_engine::model::scripted::ScriptedModel;
use agent_engine::sessions::InMemorySessions;
use agent_engine::transcript::Block;
use domain::{EngineEvent, PermissionMode, PromptRequest};
use serde_json::json;

use crate::{Source, finish, model, start};

const TOOL: &str = r#"
export const spec = {
  name: "Summarize",
  description: "Summarizes a file.",
  input: { type: "object", properties: { path: { type: "string" } } },
};
export function run(input, ctx) {
  const text = ctx.tool("Read", { file_path: input.path });
  const summary = ctx.llm({ prompt: text.includes("PERSIMMON") ? "has the codename" : "has nothing" });
  let refused = "";
  try { ctx.tool("Nope", {}) } catch (error) { refused = error.message }
  return { summary, cwd: ctx.cwd, tools: ctx.tools.includes("Read"), refused: refused !== "" };
}
"#;

#[test]
fn a_javascript_tool_reads_through_another_tool_and_asks_the_model() {
    let dir = tempfile::tempdir().expect("dir");
    let tools = dir.path().join(brand::DIR).join("tools");
    std::fs::create_dir_all(&tools).expect("tools");
    std::fs::write(tools.join("summarize.js"), TOOL).expect("tool");
    std::fs::write(dir.path().join("notes.md"), "The codename is PERSIMMON.").expect("notes");
    let model_api = Arc::new(ScriptedModel::new([
        ScriptedModel::reply(vec![Block::ToolUse {
            id: "s".into(),
            name: "Summarize".into(),
            input: json!({ "path": "notes.md" }),
        }]),
        ScriptedModel::reply(vec![Block::Text("A short summary.".into())]),
        ScriptedModel::reply(vec![Block::Text("done".into())]),
    ]));
    let engine = NativeEngine::new(
        Arc::new(Source(Mutex::new(vec![Arc::clone(&model_api)]))),
        Arc::new(InMemorySessions::default()),
    );
    let mut request = PromptRequest::plain(dir.path().into(), "Summarize the notes.", model());
    request.permission = PermissionMode::Bypass;
    request.safety_check = false;
    let events = finish(&start(&engine, &request));
    assert!(matches!(events.last(), Some(EngineEvent::Completed { .. })), "{events:?}");
    let output = events
        .iter()
        .find_map(|event| match event {
            EngineEvent::ToolResult { output, .. } if output.contains("summary") => Some(output.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("{events:?}"));
    let result: serde_json::Value = serde_json::from_str(&output).expect("json");
    assert_eq!(result["summary"], "A short summary.");
    assert_eq!(result["tools"], true);
    assert_eq!(result["refused"], true, "an unknown tool is an error the script can catch");
    let asked = model_api.asked();
    assert_eq!(asked[1].messages[0].text(), "has the codename", "the script's own model call");
    assert!(asked[0].tools.iter().any(|tool| tool.name == "Summarize"), "the model is offered the script tool");
}
