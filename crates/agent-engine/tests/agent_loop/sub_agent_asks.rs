use std::sync::Arc;

use agent_engine::agent::Agent;
use agent_engine::model::TurnRequest;
use agent_engine::model::scripted::ScriptedModel;
use agent_engine::tools;
use agent_engine::transcript::{Block, Message};
use domain::{Decision, EngineEvent, PermissionMode};
use serde_json::json;

use crate::{Rig, request, tool_use};

#[test]
fn an_ask_from_a_sub_agent_is_told_first_as_that_agent_s() {
    let rig = Rig::new(PermissionMode::AcceptEdits);
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![tool_use(
            "a1",
            "Agent",
            json!({"description": "Write it", "prompt": "Write out.txt"}),
        )]),
        ScriptedModel::reply(vec![tool_use("b1", "Bash", json!({"command": "echo hi > out.txt"}))]),
        ScriptedModel::reply(vec![Block::Text("It was not allowed.".into())]),
        ScriptedModel::reply(vec![Block::Text("Nothing written.".into())]),
    ]);
    let mut tools = tools::standard();
    tools.push(Box::new(tools::subagent::AgentTool::default()));
    let events = Arc::clone(&rig.events);
    let emit = move |event: EngineEvent| events.lock().expect("events").push(event);
    let control = Arc::clone(&rig.control);
    let pending_events = Arc::clone(&rig.events);
    let answerer = std::thread::spawn(move || {
        for _ in 0..200 {
            let asks: Vec<_> = pending_events
                .lock()
                .expect("events")
                .iter()
                .filter_map(|event| match event {
                    EngineEvent::Asked(ask) => Some(ask.id.clone()),
                    _ => None,
                })
                .collect();
            for id in asks {
                control.answer(&id, Decision::Deny);
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    });
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
    agent.run(&mut vec![Message::user("write it")]).expect("run");
    answerer.join().expect("answerer");

    let events = rig.events();
    let asked = events.iter().position(|event| matches!(event, EngineEvent::Asked(_))).expect("an ask");
    let EngineEvent::Asked(ask) = &events[asked] else { unreachable!() };
    assert!(
        matches!(&events[asked - 1], EngineEvent::AgentEvent { index: 0, event } if **event == EngineEvent::Asked(ask.clone())),
        "{:?}",
        events[asked - 1]
    );
}
