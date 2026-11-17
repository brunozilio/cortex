use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use agent_engine::agent::Ending;
use agent_engine::control::Control;
use agent_engine::model::scripted::ScriptedModel;
use agent_engine::model::{BoxFuture, CancellationToken, Delta, ModelApi, ModelError, OnDelta, Reply, TurnRequest};
use agent_engine::tools;
use agent_engine::transcript::{Block, Message};
use domain::{AgentStatus, EngineEvent, PermissionMode};
use serde_json::{Value, json};

use crate::{Rig, tool_use};

fn agent_tool() -> Vec<Box<dyn tools::Tool>> {
    vec![Box::new(tools::subagent::AgentTool::default())]
}

fn delegated(rig: &Rig, input: Value) -> (ScriptedModel, String) {
    let model = ScriptedModel::new(vec![
        ScriptedModel::reply(vec![tool_use("a1", "Agent", input)]),
        ScriptedModel::reply(vec![Block::Text("report".into())]),
        ScriptedModel::reply(vec![Block::Text("done".into())]),
    ]);
    let mut history = vec![Message::user("go")];
    rig.run_with(&model, &mut history, agent_tool()).expect("run");
    let result = history
        .iter()
        .flat_map(|message| &message.blocks)
        .find_map(|block| match block {
            Block::ToolResult { content, .. } => Some(content.clone()),
            _ => None,
        })
        .expect("the Agent call's result");
    (model, result)
}

struct StoppedMidAnswer {
    control: Arc<Control>,
    parent: AtomicUsize,
}

impl ModelApi for StoppedMidAnswer {
    fn turn<'a>(
        &'a self,
        request: &'a TurnRequest,
        on: OnDelta<'a>,
        _: &'a CancellationToken,
    ) -> BoxFuture<'a, Result<Reply, ModelError>> {
        Box::pin(async move {
            if request.system.contains("# You are a sub-agent") {
                on(Delta::Text("Half of the plan".into()));
                self.control.cancel();
                return Err(ModelError::Cancelled);
            }
            assert_eq!(self.parent.fetch_add(1, Ordering::Relaxed), 0, "a cancelled run asks nothing more");
            ScriptedModel::reply(vec![tool_use("a1", "Agent", json!({"description": "Plan", "prompt": "Plan it"}))])
        })
    }
}

#[test]
fn a_sub_agent_cancelled_mid_answer_is_cancelled_not_failed_and_hands_back_what_it_was_saying() {
    let rig = Rig::new(PermissionMode::AcceptEdits);
    let model = StoppedMidAnswer { control: Arc::clone(&rig.control), parent: AtomicUsize::new(0) };
    let mut tools = tools::standard();
    tools.extend(agent_tool());
    let events = Arc::clone(&rig.events);
    let emit = move |event: EngineEvent| events.lock().expect("events").push(event);
    let agent = agent_engine::agent::Agent {
        api: &model,
        tools: &tools,
        workspace: &rig.workspace,
        control: &rig.control,
        emit: &emit,
        request: TurnRequest { tools: agent_engine::agent::Agent::specs(&tools), ..crate::request() },
        agents: &[],
        depth: 0,
        turn_limit: agent_engine::agent::MAX_TURNS,
    };
    let ending = agent.run(&mut vec![Message::user("go")]).map(|_| ());
    assert_eq!(ending, Err(Ending::Cancelled));

    let events = rig.events();
    let statuses: Vec<&AgentStatus> = events
        .iter()
        .filter_map(|event| match event {
            EngineEvent::AgentUpdate { status, .. } => Some(status),
            _ => None,
        })
        .collect();
    assert_eq!(statuses.last(), Some(&&AgentStatus::Cancelled));
    let result = events
        .iter()
        .find_map(|event| match event {
            EngineEvent::ToolResult { id, output, .. } if id == "a1" => Some(output.as_str()),
            _ => None,
        })
        .expect("the Agent call's result");
    assert!(result.contains("Cancelled") && result.contains("Half of the plan"), "{result}");
}

#[test]
fn a_sub_agent_started_while_planning_is_offered_only_the_tools_planning_allows() {
    let offered = |mode: PermissionMode| {
        let (model, _) = delegated(&Rig::new(mode), json!({"description": "Look", "prompt": "Look around"}));
        model.asked()[1].tools.iter().map(|spec| spec.name.clone()).collect::<Vec<_>>()
    };
    let has = |names: &[String], name: &str| names.iter().any(|offered| offered == name);

    let planning = offered(PermissionMode::Plan);
    assert!(has(&planning, "Read") && has(&planning, "Grep") && has(&planning, "Glob"), "{planning:?}");
    assert!(!has(&planning, "Bash") && !has(&planning, "Write") && !has(&planning, "WebFetch"), "{planning:?}");

    let editing = offered(PermissionMode::AcceptEdits);
    assert!(has(&editing, "Bash") && has(&editing, "Write"), "{editing:?}");
}

#[test]
fn the_report_says_when_the_model_asked_for_was_not_used() {
    agent_engine::providers::install_testing();
    let rig = Rig::new(PermissionMode::AcceptEdits);
    let input = json!({"description": "Look", "prompt": "Say hi", "model": "some-other-vendor-model"});
    let (_, result) = delegated(&rig, input);
    assert!(result.starts_with("report"), "{result}");
    assert!(result.contains("some-other-vendor-model is not a model this provider serves; ran on m"), "{result}");

    let (_, plain) = delegated(&rig, json!({"description": "Look", "prompt": "Say hi"}));
    assert!(!plain.contains("is not a model"), "{plain}");
}
