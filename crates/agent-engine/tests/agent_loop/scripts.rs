use std::sync::Arc;

use agent_engine::model::scripted::ScriptedModel;
use agent_engine::tools::Tool;
use agent_engine::tools::script_tool::load;
use agent_engine::transcript::{Block, Message};
use domain::{Decision, EngineEvent, PermissionMode};
use serde_json::json;

use crate::{Rig, tool_use};

const TOOL: &str = r#"
export const spec = { name: "Mark", description: "Writes a mark.", access: "read" };
export function run(input, ctx) {
  ctx.log("marking");
  ctx.writeFile("mark.txt", "marked");
  return "done";
}
"#;

fn script_tools(home: &std::path::Path, project: &std::path::Path) -> Vec<Box<dyn Tool>> {
    let trusted = agent_engine::trust::Gate::at(project, Some(home), domain::trust::Trust::Trusted);
    load(&trusted).into_iter().map(|tool| Box::new(tool) as Box<dyn Tool>).collect()
}

fn answer_every_ask(rig: &Rig, decision: Decision) -> std::thread::JoinHandle<usize> {
    let control = Arc::clone(&rig.control);
    let events = Arc::clone(&rig.events);
    std::thread::spawn(move || {
        let mut answered = Vec::new();
        for _ in 0..200 {
            let asks: Vec<domain::AskId> = events
                .lock()
                .expect("events")
                .iter()
                .filter_map(|event| match event {
                    EngineEvent::Asked(ask) => Some(ask.id.clone()),
                    _ => None,
                })
                .collect();
            for id in asks {
                if !answered.contains(&id) {
                    answered.push(id.clone());
                    control.answer(&id, decision.clone());
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        answered.len()
    })
}

fn model() -> ScriptedModel {
    ScriptedModel::new([
        ScriptedModel::reply(vec![tool_use("m", "Mark", json!({}))]),
        ScriptedModel::reply(vec![Block::Text("Ok.".into())]),
    ])
}

#[test]
fn a_project_s_tool_asks_the_first_time_even_when_bypassing_and_a_refusal_stops_it() {
    let rig = Rig::new(PermissionMode::Bypass);
    let home = tempfile::tempdir().expect("home");
    let folder = rig.dir.path().join(brand::DIR).join("tools");
    std::fs::create_dir_all(&folder).expect("tools");
    std::fs::write(folder.join("mark.js"), TOOL).expect("tool");
    let answerer = answer_every_ask(&rig, Decision::Deny);
    rig.run_with(&model(), &mut vec![Message::user("mark")], script_tools(home.path(), rig.dir.path())).expect("run");
    assert_eq!(answerer.join().expect("answerer"), 1, "asked once");
    assert!(!rig.dir.path().join("mark.txt").exists(), "a refused tool never runs");
}

fn project_tool_run(rig: &Rig) -> String {
    let home = tempfile::tempdir().expect("home");
    let folder = rig.dir.path().join(brand::DIR).join("tools");
    std::fs::create_dir_all(&folder).expect("tools");
    std::fs::write(folder.join("mark.js"), TOOL).expect("tool");
    let answerer = answer_every_ask(rig, Decision::Allow);
    let mut history = vec![Message::user("mark")];
    rig.run_with(&model(), &mut history, script_tools(home.path(), rig.dir.path())).expect("run");
    assert_eq!(answerer.join().expect("answerer"), 1, "asked once");
    format!("{:?}", history.iter().flat_map(|message| &message.blocks).collect::<Vec<_>>())
}

#[test]
fn a_project_s_tool_the_user_allowed_still_gets_no_files_by_default() {
    let rig = Rig::new(PermissionMode::Ask);
    let said = project_tool_run(&rig);
    assert!(!rig.dir.path().join("mark.txt").exists(), "allowed to run, not to write");
    assert!(said.contains("scripts.files"), "the script is told which setting grants it: {said}");
}

#[test]
fn a_project_s_tool_writes_once_the_owner_s_settings_grant_files() {
    let mut rig = Rig::new(PermissionMode::Ask);
    rig.workspace.policy.scripts.project.files = true;
    project_tool_run(&rig);
    assert_eq!(std::fs::read_to_string(rig.dir.path().join("mark.txt")).expect("mark"), "marked");
}

#[test]
fn the_user_s_own_tool_runs_without_asking_and_its_log_reaches_the_screen() {
    let rig = Rig::new(PermissionMode::Bypass);
    let home = tempfile::tempdir().expect("home");
    let folder = home.path().join(brand::DIR).join("tools");
    std::fs::create_dir_all(&folder).expect("tools");
    std::fs::write(folder.join("mark.js"), TOOL).expect("tool");
    rig.run_with(&model(), &mut vec![Message::user("mark")], script_tools(home.path(), rig.dir.path())).expect("run");
    assert_eq!(std::fs::read_to_string(rig.dir.path().join("mark.txt")).expect("mark"), "marked");
    let events = rig.events();
    assert!(!events.iter().any(|event| matches!(event, EngineEvent::Asked(_))));
    assert!(events.contains(&EngineEvent::ToolLog { id: "m".into(), text: "marking".into() }), "{events:?}");
}

#[test]
fn a_tool_that_exports_execute_runs_it_and_has_no_built_in_to_call() {
    let rig = Rig::new(PermissionMode::Bypass);
    let home = tempfile::tempdir().expect("home");
    let folder = home.path().join(brand::DIR).join("tools");
    std::fs::create_dir_all(&folder).expect("tools");
    let tool = r#"
export const spec = { name: "Mark", description: "Writes a mark.", access: "read" };
export function execute(input, ctx) {
  ctx.writeFile("mark.txt", "executed");
  try { ctx.builtin(input) } catch (error) { return error.message }
}
"#;
    std::fs::write(folder.join("mark.js"), tool).expect("tool");
    let mut history = vec![Message::user("mark")];
    rig.run_with(&model(), &mut history, script_tools(home.path(), rig.dir.path())).expect("run");
    assert_eq!(std::fs::read_to_string(rig.dir.path().join("mark.txt")).expect("mark"), "executed");
    let answered = history.iter().flat_map(|message| message.blocks.iter()).any(
        |block| matches!(block, Block::ToolResult { content, .. } if content.contains("Mark has no built-in to call")),
    );
    assert!(answered, "{history:?}");
}

fn own_tool(home: &std::path::Path, file: &str, source: &str) {
    let folder = home.join(brand::DIR).join("tools");
    std::fs::create_dir_all(&folder).expect("tools");
    std::fs::write(folder.join(file), source).expect("tool");
}

fn results(history: &[Message]) -> Vec<&str> {
    history
        .iter()
        .flat_map(|message| message.blocks.iter())
        .filter_map(|block| match block {
            Block::ToolResult { content, .. } => Some(content.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_tool_a_script_calls_again_and_again_is_held_back_like_the_model_s_own_calls() {
    let rig = Rig::new(PermissionMode::Bypass);
    let home = tempfile::tempdir().expect("home");
    let tool = r#"
export const spec = { name: "Thrice", description: "Reads one file three times.", access: "read" };
export function run(input, ctx) {
  const read = () => ctx.tool("Read", { file_path: "notes.txt" });
  read();
  read();
  try { read() } catch (error) { return "held back: " + error.message }
  return "read three times";
}
"#;
    own_tool(home.path(), "thrice.js", tool);
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![tool_use("t", "Thrice", json!({}))]),
        ScriptedModel::reply(vec![Block::Text("Ok.".into())]),
    ]);
    let mut history = vec![Message::user("read")];
    rig.run_with(&model, &mut history, script_tools(home.path(), rig.dir.path())).expect("run");
    let said = results(&history);
    assert!(said[0].starts_with("held back: ") && said[0].contains("you already called Read"), "{said:?}");
    assert!(rig.events().contains(&EngineEvent::Repeated { tool: "Read".into() }));
}

#[test]
fn a_background_agent_a_script_starts_is_one_the_run_can_collect() {
    let rig = Rig::new(PermissionMode::Bypass);
    let home = tempfile::tempdir().expect("home");
    let tool = r#"
export const spec = { name: "Delegate", description: "Starts a reviewer in the background.", access: "read" };
export function run(input, ctx) {
  return ctx.tool("Agent", { description: "Review", prompt: "review", run_in_background: true });
}
"#;
    own_tool(home.path(), "delegate.js", tool);
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![tool_use("d", "Delegate", json!({}))]),
        ScriptedModel::reply(vec![Block::Text("REVIEWED".into())]),
        ScriptedModel::reply(vec![tool_use("o", "TaskOutput", json!({"task_id": "task-1"}))]),
        ScriptedModel::reply(vec![Block::Text("Ok.".into())]),
    ]);
    let mut extra: Vec<Box<dyn Tool>> = vec![Box::new(agent_engine::tools::subagent::AgentTool::default())];
    extra.extend(agent_engine::tools::task::tools());
    extra.extend(script_tools(home.path(), rig.dir.path()));
    let mut history = vec![Message::user("review")];
    rig.run_with(&model, &mut history, extra).expect("run");
    let said = results(&history);
    assert!(said[0].contains(r#""task_id":"task-1""#), "the script hands back the task it started: {said:?}");
    assert!(said[1].starts_with("REVIEWED"), "the run knows that task and collects its report: {said:?}");
}
