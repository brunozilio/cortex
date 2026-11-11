//! SendMessage: a sub-agent of this run continues with its conversation
//! intact.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use agent_engine::agent::Agent;
use agent_engine::model::scripted::ScriptedModel;
use agent_engine::model::{Delta, ModelApi, ModelError, Reply, TurnRequest};
use agent_engine::tools;
use agent_engine::transcript::{Block, Message};
use domain::{AgentStatus, EngineEvent, PermissionMode};
use serde_json::json;

use crate::{Rig, request, tool_use};

fn run(rig: &Rig, model: &dyn ModelApi) -> Vec<Message> {
    let mut tools = tools::standard();
    tools.push(Box::new(tools::subagent::AgentTool::default()));
    tools.push(Box::new(tools::send_message::SendMessageTool));
    let events = Arc::clone(&rig.events);
    let emit = move |event: EngineEvent| events.lock().expect("events").push(event);
    let agent = Agent {
        api: model,
        tools: &tools,
        workspace: &rig.workspace,
        control: &rig.control,
        emit: &emit,
        request: TurnRequest { tools: Agent::specs(&tools), ..request() },
        agents: &[],
        depth: 0,
        turn_limit: agent_engine::agent::MAX_TURNS,
    };
    let mut history = vec![Message::user("look at the notes")];
    agent.run(&mut history).expect("run");
    history
}

fn result(history: &[Message], wanted: &str) -> (String, bool) {
    history
        .iter()
        .flat_map(|m| &m.blocks)
        .find_map(|block| match block {
            Block::ToolResult { id, content, is_error } if id == wanted => Some((content.clone(), *is_error)),
            _ => None,
        })
        .expect("tool result")
}

#[test]
fn a_message_continues_the_sub_agent_in_its_own_conversation() {
    let rig = Rig::new(PermissionMode::AcceptEdits);
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![tool_use("a1", "Agent", json!({"description": "Notes", "prompt": "Read notes"}))]),
        ScriptedModel::reply(vec![Block::Text("They list alpha and beta.".into())]),
        ScriptedModel::reply(vec![tool_use(
            "m1",
            "SendMessage",
            json!({"to": "agent-1", "message": "Which comes first?"}),
        )]),
        ScriptedModel::reply(vec![Block::Text("Alpha.".into())]),
        ScriptedModel::reply(vec![Block::Text("Alpha comes first.".into())]),
    ]);

    let history = run(&rig, &model);

    let (first, _) = result(&history, "a1");
    assert!(first.starts_with("They list alpha and beta."), "{first}");
    assert!(first.contains("agent-1"), "the report names the agent to message: {first}");
    let (second, failed) = result(&history, "m1");
    assert!(!failed, "{second}");
    assert!(second.starts_with("Alpha."), "{second}");
    let continued = &model.asked()[3].messages;
    let texts: Vec<String> = continued.iter().map(Message::text).collect();
    assert_eq!(texts, ["Read notes", "They list alpha and beta.", "Which comes first?"]);
    let events = rig.events();
    let spawned = events.iter().filter(|e| matches!(e, EngineEvent::AgentSpawned { .. })).count();
    assert_eq!(spawned, 1, "the continued agent keeps its card");
    let done = events
        .iter()
        .filter(|e| matches!(e, EngineEvent::AgentUpdate { index: 0, status: AgentStatus::Done { .. }, .. }))
        .count();
    assert_eq!(done, 2);
}

#[test]
fn an_unknown_agent_is_an_error_and_sub_agents_are_not_offered_the_tool() {
    let rig = Rig::new(PermissionMode::AcceptEdits);
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![tool_use("m1", "SendMessage", json!({"to": "agent-9", "message": "Hi"}))]),
        ScriptedModel::reply(vec![tool_use("a1", "Agent", json!({"description": "Notes", "prompt": "Read notes"}))]),
        ScriptedModel::reply(vec![Block::Text("Done.".into())]),
        ScriptedModel::reply(vec![Block::Text("Finished.".into())]),
    ]);

    let history = run(&rig, &model);

    let (error, failed) = result(&history, "m1");
    assert!(failed);
    assert!(error.starts_with("No agent or session agent-9"), "{error}");
    let child = &model.asked()[2];
    assert!(!child.tools.iter().any(|spec| spec.name == "SendMessage" || spec.name == "Agent"));
}

/// The parent's and the sub-agents' replies apart, for agents that run
/// alongside the parent.
struct Split {
    parent: ScriptedModel,
    child: ScriptedModel,
}

impl ModelApi for Split {
    fn turn(&self, request: &TurnRequest, on: &mut dyn FnMut(Delta), stop: &AtomicBool) -> Result<Reply, ModelError> {
        let script = if request.system.contains("# You are a sub-agent") { &self.child } else { &self.parent };
        script.turn(request, on, stop)
    }
}

#[test]
fn a_background_agent_takes_messages_once_collected() {
    let rig = Rig::new(PermissionMode::AcceptEdits);
    let model = Split {
        parent: ScriptedModel::new([
            ScriptedModel::reply(vec![tool_use(
                "a1",
                "Agent",
                json!({"description": "Notes", "prompt": "Read notes", "run_in_background": true}),
            )]),
            ScriptedModel::reply(vec![tool_use("out", "TaskOutput", json!({"task_id": "task-1"}))]),
            ScriptedModel::reply(vec![tool_use(
                "m1",
                "SendMessage",
                json!({"to": "agent-1", "message": "Count them"}),
            )]),
            ScriptedModel::reply(vec![Block::Text("There are two.".into())]),
        ]),
        child: ScriptedModel::new([
            ScriptedModel::reply(vec![Block::Text("Alpha and beta.".into())]),
            ScriptedModel::reply(vec![Block::Text("Two.".into())]),
        ]),
    };

    let history = run(&rig, &model);

    let (collected, _) = result(&history, "out");
    assert!(collected.contains("agent_id: agent-1"), "{collected}");
    let (second, failed) = result(&history, "m1");
    assert!(!failed && second.starts_with("Two."), "{second}");
}
