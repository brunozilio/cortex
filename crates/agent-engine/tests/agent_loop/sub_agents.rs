use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use agent_engine::agent::Agent;
use agent_engine::model::scripted::ScriptedModel;
use agent_engine::model::{ModelError, TurnRequest};
use agent_engine::tools;
use agent_engine::transcript::{Block, Message};
use domain::{EngineEvent, PermissionMode};
use serde_json::json;

use crate::{Rig, request, tool_use};

#[test]
fn a_sub_agent_works_in_its_own_conversation_and_reports_back() {
    let rig = Rig::new(PermissionMode::AcceptEdits);
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![tool_use(
            "a1",
            "Agent",
            json!({"description": "Read the notes", "prompt": "What does notes.txt say?"}),
        )]),
        ScriptedModel::reply(vec![tool_use("s1", "Read", json!({"file_path": "notes.txt"}))]),
        ScriptedModel::reply(vec![Block::Text("It lists alpha and beta.".into())]),
        ScriptedModel::reply(vec![Block::Text("The notes list alpha and beta.".into())]),
    ]);
    let mut tools = tools::standard();
    tools.push(Box::new(tools::subagent::AgentTool::default()));
    let events = Arc::clone(&rig.events);
    let emit = move |event: EngineEvent| events.lock().expect("events").push(event);
    let agent = Agent {
        api: &model,
        tools: &tools,
        workspace: &rig.workspace,
        control: &rig.control,
        emit: &emit,
        request: TurnRequest { tools: Agent::specs(&tools), ..request() },
        agents: &[],
        depth: 0,
        turn_limit: agent_engine::agent::MAX_TURNS,
    };
    let mut history = vec![Message::user("summarise the notes")];
    agent.run(&mut history).expect("run");

    let report = history.iter().flat_map(|m| &m.blocks).find_map(|block| match block {
        Block::ToolResult { id, content, .. } if id == "a1" => Some(content.clone()),
        _ => None,
    });
    assert_eq!(report.as_deref(), Some("It lists alpha and beta.\n\nagent_id: agent-1 (SendMessage continues it)"));
    assert_eq!(history.len(), 4, "the sub-agent's turns stay in its own conversation");
    let events = rig.events();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, EngineEvent::AgentSpawned { index: 0, kind, .. } if kind == "general-purpose"))
    );
    assert!(events.iter().any(|e| matches!(e, EngineEvent::AgentEvent { index: 0, event } if matches!(**event, EngineEvent::ToolStarted(ref call) if call.name == "Read"))));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, EngineEvent::AgentUpdate { index: 0, status: domain::AgentStatus::Done { .. }, .. }))
    );
    assert!(
        !events.iter().any(|e| matches!(e, EngineEvent::ToolStarted(call) if call.id == "s1")),
        "the sub-agent's steps are its card's, not the run's"
    );
}

#[test]
fn a_call_can_pick_the_sub_agent_s_model() {
    agent_engine::providers::install_testing();
    let mut rig = Rig::new(PermissionMode::AcceptEdits);
    rig.workspace.model.provider = domain::provider::ModelProvider::new("family-key");
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![tool_use(
            "a1",
            "Agent",
            json!({"description": "Quick look", "prompt": "Say hi", "model": "haiku"}),
        )]),
        ScriptedModel::reply(vec![Block::Text("hi".into())]),
        ScriptedModel::reply(vec![Block::Text("Done.".into())]),
    ]);
    let mut tools = tools::standard();
    tools.push(Box::new(tools::subagent::AgentTool::default()));
    let emit = |_: EngineEvent| {};
    let agent = Agent {
        api: &model,
        tools: &tools,
        workspace: &rig.workspace,
        control: &rig.control,
        emit: &emit,
        request: TurnRequest { tools: Agent::specs(&tools), ..request() },
        agents: &[],
        depth: 0,
        turn_limit: agent_engine::agent::MAX_TURNS,
    };
    agent.run(&mut vec![Message::user("go")]).expect("run");
    assert_eq!(model.asked()[1].model, "claude-haiku-5-5", "the sub-agent's turn used the model it was given");
    assert_eq!(model.asked()[2].model, "m", "the parent keeps its own");
}

#[test]
fn background_agent_runs_concurrently_and_output_is_collected() {
    use agent_engine::model::{Delta, ModelApi, Reply};
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Concurrent {
        parent: AtomicUsize,
        started: AtomicBool,
        done: AtomicBool,
    }
    impl ModelApi for Concurrent {
        fn turn(&self, request: &TurnRequest, _: &mut dyn FnMut(Delta), _: &AtomicBool) -> Result<Reply, ModelError> {
            if request.system.contains("# You are a sub-agent") {
                self.started.store(true, Ordering::SeqCst);
                std::thread::sleep(std::time::Duration::from_millis(150));
                self.done.store(true, Ordering::SeqCst);
                return ScriptedModel::reply(vec![Block::Text("independent report".into())]);
            }
            match self.parent.fetch_add(1, Ordering::SeqCst) {
                0 => ScriptedModel::reply(vec![tool_use(
                    "a",
                    "Agent",
                    json!({"description":"Review independently","prompt":"review","run_in_background":true}),
                )]),
                1 => {
                    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
                    while !self.started.load(Ordering::SeqCst) && std::time::Instant::now() < deadline {
                        std::thread::yield_now();
                    }
                    assert!(self.started.load(Ordering::SeqCst));
                    assert!(!self.done.load(Ordering::SeqCst), "parent must advance before child completion");
                    ScriptedModel::reply(vec![tool_use("out", "TaskOutput", json!({"task_id":"task-1"}))])
                }
                _ => {
                    assert!(request.messages.iter().any(|m| m.blocks.iter().any(
                        |b| matches!(b, Block::ToolResult {id,content,..} if id=="out" && content.starts_with("independent report\n\nagent_id: agent-"))
                    )));
                    ScriptedModel::reply(vec![Block::Text("Reviewed".into())])
                }
            }
        }
    }
    let rig = Rig::new(PermissionMode::Bypass);
    let model =
        Concurrent { parent: AtomicUsize::new(0), started: AtomicBool::new(false), done: AtomicBool::new(false) };
    let mut tools = tools::standard();
    tools.push(Box::new(tools::subagent::AgentTool::default()));
    tools.extend(tools::task::tools());
    let events = rig.events.clone();
    let emit = move |event| events.lock().expect("events").push(event);
    let agent = Agent {
        api: &model,
        tools: &tools,
        workspace: &rig.workspace,
        control: &rig.control,
        emit: &emit,
        request: TurnRequest { tools: Agent::specs(&tools), ..request() },
        agents: &[],
        depth: 0,
        turn_limit: agent_engine::agent::MAX_TURNS,
    };
    agent.run(&mut vec![Message::user("review")]).expect("background run");
    assert_eq!(model.parent.load(Ordering::SeqCst), 3);
    assert!(model.done.load(Ordering::SeqCst));
}

#[test]
#[cfg(unix)]
fn stopping_background_task_joins_its_shell_without_stopping_parent() {
    use agent_engine::model::{Delta, ModelApi, Reply};
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct StopModel {
        parent: AtomicUsize,
        child: AtomicUsize,
        dir: std::path::PathBuf,
    }
    impl ModelApi for StopModel {
        fn turn(&self, request: &TurnRequest, _: &mut dyn FnMut(Delta), _: &AtomicBool) -> Result<Reply, ModelError> {
            if request.system.contains("# You are a sub-agent") {
                assert_eq!(self.child.fetch_add(1, Ordering::Relaxed), 0, "cancelled child must not call model again");
                return ScriptedModel::reply(vec![tool_use(
                    "slow",
                    "Bash",
                    json!({"command":"echo ready > ready; sleep 1; echo late > late"}),
                )]);
            }
            match self.parent.fetch_add(1, Ordering::Relaxed) {
                0 => ScriptedModel::reply(vec![tool_use(
                    "spawn",
                    "Agent",
                    json!({"description":"Controlled cancellation","prompt":"execute fixture","run_in_background":true}),
                )]),
                1 => {
                    for _ in 0..200 {
                        if self.dir.join("ready").exists() {
                            break;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(5));
                    }
                    assert!(self.dir.join("ready").exists());
                    ScriptedModel::reply(vec![tool_use("stop", "TaskStop", json!({"task_id":"task-1"}))])
                }
                _ => ScriptedModel::reply(vec![Block::Text("Parent continued.".into())]),
            }
        }
    }
    let rig = Rig::new(PermissionMode::Bypass);
    let model = StopModel { parent: AtomicUsize::new(0), child: AtomicUsize::new(0), dir: rig.dir.path().into() };
    let mut tools = tools::standard();
    tools.push(Box::new(tools::subagent::AgentTool::default()));
    tools.extend(tools::task::tools());
    let events = rig.events.clone();
    let emit = move |event| events.lock().expect("valid test fixture").push(event);
    let agent = Agent {
        api: &model,
        tools: &tools,
        workspace: &rig.workspace,
        control: &rig.control,
        emit: &emit,
        request: TurnRequest { tools: Agent::specs(&tools), ..request() },
        agents: &[],
        depth: 0,
        turn_limit: agent_engine::agent::MAX_TURNS,
    };
    let mut history = vec![Message::user("controlled test")];
    agent.run(&mut history).expect("valid test fixture");
    assert!(!rig.control.is_stopped());
    assert_eq!(history.last().expect("valid test fixture").text(), "Parent continued.");
    std::thread::sleep(std::time::Duration::from_millis(1100));
    assert!(!rig.dir.path().join("late").exists());
}
