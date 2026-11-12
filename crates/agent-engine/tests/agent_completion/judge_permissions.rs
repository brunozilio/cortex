use std::sync::{Arc, Mutex};

use agent_engine::agent::Agent;
use agent_engine::control::Control;
use agent_engine::model::TurnRequest;
use agent_engine::model::scripted::ScriptedModel;
use agent_engine::tools::{self, Verification, Workspace};
use agent_engine::transcript::Message;
use domain::permission_rules::PermissionRules;
use domain::{Criteria, Effort, EngineEvent, PermissionMode};
use serde_json::json;

use super::{call, done};

const PASS: &str = r#"{"passed":true,"checks":[]}"#;
const CRITERIA: &str = "`echo checked` prints checked";

fn judged(dir: &std::path::Path) -> Workspace {
    let mut workspace = Workspace::new(dir);
    workspace.verification = Some(Verification {
        criteria: Criteria::user(CRITERIA),
        model: "claude-opus-5-5".into(),
        allow: domain::verification::judge_rules(CRITERIA, &PermissionRules::default()),
    });
    workspace
}

fn judge_runs(command: &str, mode: PermissionMode, workspace: &Workspace) -> Vec<EngineEvent> {
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![call("look", "Glob", json!({"pattern": "*"}))]),
        done("Done"),
        ScriptedModel::reply(vec![call("check", "Bash", json!({ "command": command }))]),
        done(PASS),
    ]);
    let tools = tools::standard();
    let control = Control::new(mode, Arc::default());
    let events = Mutex::new(Vec::new());
    let emit = |event| events.lock().expect("events").push(event);
    let agent = Agent {
        api: &model,
        tools: &tools,
        workspace,
        control: &control,
        emit: &emit,
        request: TurnRequest {
            model: "claude-opus-5-5".into(),
            system: "Parent context".into(),
            messages: vec![],
            tools: Agent::specs(&tools),
            effort: Effort::Medium,
            max_tokens: 8192,
        },
        agents: &[],
        depth: 0,
        turn_limit: 20,
    };
    agent.run(&mut vec![Message::user("check it")]).expect("run completes");
    assert_eq!(model.asked().len(), 4, "no other model call, such as the safety check, took a reply");
    events.into_inner().expect("events")
}

fn judge_result(events: &[EngineEvent]) -> (bool, String) {
    events
        .iter()
        .find_map(|event| match event {
            EngineEvent::AgentEvent { event, .. } => match event.as_ref() {
                EngineEvent::ToolResult { output, failed, .. } => Some((*failed, output.clone())),
                _ => None,
            },
            _ => None,
        })
        .expect("the judge's command result")
}

fn asked(events: &[EngineEvent]) -> bool {
    events.iter().any(|event| matches!(event, EngineEvent::Asked(_)))
}

#[test]
fn the_judge_runs_a_command_the_criteria_name_without_asking() {
    let dir = tempfile::tempdir().expect("dir");
    let events = judge_runs("echo checked", PermissionMode::AcceptEdits, &judged(dir.path()));
    let (failed, output) = judge_result(&events);
    assert!(!failed && output.contains("checked"), "{output}");
    assert!(!asked(&events));
}

#[test]
fn outside_bypass_the_judge_is_refused_any_other_command_without_asking() {
    let dir = tempfile::tempdir().expect("dir");
    let events = judge_runs("touch made.txt", PermissionMode::AcceptEdits, &judged(dir.path()));
    let (failed, output) = judge_result(&events);
    assert!(failed && output.contains("requires permission"), "{output}");
    assert!(!asked(&events));
    assert!(!dir.path().join("made.txt").exists());
}

#[test]
fn in_bypass_a_command_the_criteria_name_skips_the_safety_check() {
    let dir = tempfile::tempdir().expect("dir");
    let mut workspace = judged(dir.path());
    workspace.safety = Some(Arc::new(agent_engine::guard::Safety::default()));
    let events = judge_runs("echo checked", PermissionMode::Bypass, &workspace);
    let (failed, output) = judge_result(&events);
    assert!(!failed && output.contains("checked"), "{output}");
}
