//! The judge: a top-level run with verification criteria is graded before
//! it completes, and tries again with the judge's reasons when it fails.

use std::sync::{Arc, Mutex};

use serde_json::json;
use superfik_core::{AgentStatus, Criteria, Effort, EngineEvent, PermissionMode, Verdict};
use superfik_native::{
    agent::Agent,
    control::Control,
    judge::Verification,
    model::{TurnRequest, scripted::ScriptedModel},
    tools::{self, Workspace, subagent::AgentTool},
    transcript::Message,
};

use super::{call, done};

const PASS: &str = r#"{"passed":true,"checks":[{"criterion":"hello.txt says oi","passed":true,"evidence":"cat: oi"}]}"#;
const FAIL: &str =
    r#"{"passed":false,"checks":[{"criterion":"hello.txt says oi","passed":false,"evidence":"cat: ola"}]}"#;

fn judged(workspace: &mut Workspace) {
    workspace.verification =
        Some(Verification { criteria: Criteria::user("hello.txt says oi"), model: "claude-opus-5-5".into() });
}

fn run(model: &ScriptedModel, workspace: &Workspace) -> Vec<EngineEvent> {
    let mut tools = tools::standard();
    tools.push(Box::new(AgentTool { agents: vec![] }));
    tools.push(Box::new(tools::skill::SkillTool::default()));
    let control = Control::new(PermissionMode::Bypass, Arc::default());
    let events = Mutex::new(Vec::new());
    let emit = |event| events.lock().expect("events").push(event);
    let agent = Agent {
        api: model,
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
    let mut history = vec![Message::user("write hello.txt")];
    agent.run(&mut history).expect("run completes");
    events.into_inner().expect("events")
}

fn verdicts(events: &[EngineEvent]) -> Vec<Verdict> {
    events
        .iter()
        .filter_map(|event| if let EngineEvent::Verified(v) = event { Some(v.clone()) } else { None })
        .collect()
}

#[test]
fn a_failed_verification_sends_the_agent_back_with_the_judges_reasons() {
    let dir = tempfile::tempdir().expect("dir");
    let mut workspace = Workspace::new(dir.path());
    judged(&mut workspace);
    let model = ScriptedModel::new([done("Wrote ola"), done(FAIL), done("Fixed to oi"), done(PASS)]);
    let events = run(&model, &workspace);
    let verdicts = verdicts(&events);
    assert_eq!(verdicts.iter().map(|v| (v.passed, v.attempt)).collect::<Vec<_>>(), vec![(false, 1), (true, 2)]);
    let asked = model.asked();
    assert_eq!(asked.len(), 4);
    let judge = &asked[1];
    let mut tools: Vec<_> = judge.tools.iter().map(|spec| spec.name.as_str()).collect();
    tools.sort_unstable();
    assert_eq!(tools, ["Bash", "Glob", "Grep", "Read", "Skill"], "the judge checks, it never edits");
    let brief = judge.messages[0].text();
    assert!(brief.contains("hello.txt says oi") && brief.contains("Wrote ola"), "{brief}");
    let retry = asked[2].messages.last().expect("feedback").text();
    assert!(retry.contains("Verification failed (attempt 1/3)") && retry.contains("cat: ola"), "{retry}");
}

#[test]
fn the_run_completes_after_the_last_failed_attempt() {
    let dir = tempfile::tempdir().expect("dir");
    let mut workspace = Workspace::new(dir.path());
    judged(&mut workspace);
    let model = ScriptedModel::new([done("a"), done(FAIL), done("b"), done(FAIL), done("c"), done(FAIL)]);
    let verdicts = verdicts(&run(&model, &workspace));
    assert_eq!(verdicts.iter().map(|v| v.attempt).collect::<Vec<_>>(), vec![1, 2, 3]);
    assert!(verdicts.iter().all(|v| !v.passed));
    assert_eq!(model.asked().len(), 6);
}

#[test]
fn without_criteria_nothing_is_judged() {
    let dir = tempfile::tempdir().expect("dir");
    let workspace = Workspace::new(dir.path());
    let model = ScriptedModel::new([done("Done")]);
    assert!(verdicts(&run(&model, &workspace)).is_empty());
    assert_eq!(model.asked().len(), 1);
}

#[test]
fn a_judge_without_a_verdict_fails_the_check_but_not_the_run() {
    let dir = tempfile::tempdir().expect("dir");
    let mut workspace = Workspace::new(dir.path());
    judged(&mut workspace);
    let model = ScriptedModel::new([done("Done"), done("Looks fine to me")]);
    let verdicts = verdicts(&run(&model, &workspace));
    assert_eq!(verdicts.len(), 1);
    assert!(!verdicts[0].passed && verdicts[0].checks[0].evidence.contains("no JSON verdict"));
    assert_eq!(model.asked().len(), 2, "an unusable judge is not retried");
}

#[test]
fn the_judge_runs_commands_and_the_user_watches_it_as_an_agent() {
    let dir = tempfile::tempdir().expect("dir");
    let mut workspace = Workspace::new(dir.path());
    judged(&mut workspace);
    let model = ScriptedModel::new([
        done("Done"),
        ScriptedModel::reply(vec![call("check", "Bash", json!({"command":"echo checked"}))]),
        done(PASS),
    ]);
    let events = run(&model, &workspace);
    assert!(events.iter().any(|e| matches!(e, EngineEvent::AgentSpawned { kind, .. } if kind == "verification")));
    assert!(events.iter().any(|e| matches!(e, EngineEvent::AgentEvent { event, .. }
        if matches!(event.as_ref(), EngineEvent::ToolStarted(tool) if tool.name == "Bash"))));
    assert!(events.iter().any(|e| matches!(e, EngineEvent::AgentEvent { event, .. }
        if matches!(event.as_ref(), EngineEvent::ToolResult { output, .. } if output.contains("checked")))));
    assert!(events.iter().any(|e| matches!(e, EngineEvent::AgentUpdate { status: AgentStatus::Done { .. }, .. })));
    assert!(verdicts(&events)[0].passed);
}

#[test]
fn a_sub_agents_work_is_not_judged_on_its_own() {
    let dir = tempfile::tempdir().expect("dir");
    let mut workspace = Workspace::new(dir.path());
    judged(&mut workspace);
    let delegated =
        ScriptedModel::reply(vec![call("sub", "Agent", json!({"description":"Write","prompt":"write it"}))]);
    let model = ScriptedModel::new([delegated, done("Sub report"), done("Final"), done(PASS)]);
    let verdicts = verdicts(&run(&model, &workspace));
    assert_eq!(verdicts.len(), 1, "only the top-level run is judged");
    assert_eq!(model.asked().len(), 4);
}
