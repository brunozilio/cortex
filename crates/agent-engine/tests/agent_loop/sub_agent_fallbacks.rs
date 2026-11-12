use agent_engine::agent::Agent;
use agent_engine::model::TurnRequest;
use agent_engine::model::scripted::ScriptedModel;
use agent_engine::tools;
use agent_engine::transcript::{Block, Message};
use domain::{EngineEvent, PermissionMode};
use serde_json::{Value, json};

use crate::{Rig, request, tool_use};

fn delegate(rig: &Rig, input: Value, replies: usize) -> (ScriptedModel, Vec<Message>) {
    let mut script = vec![ScriptedModel::reply(vec![tool_use("a1", "Agent", input)])];
    script.extend((0..replies).map(|_| ScriptedModel::reply(vec![Block::Text("ok".into())])));
    let model = ScriptedModel::new(script);
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
    let mut history = vec![Message::user("go")];
    agent.run(&mut history).expect("run");
    (model, history)
}

fn agent_result(history: &[Message]) -> (bool, String) {
    history
        .iter()
        .flat_map(|message| &message.blocks)
        .find_map(|block| match block {
            Block::ToolResult { content, is_error, .. } => Some((*is_error, content.clone())),
            _ => None,
        })
        .expect("the Agent call's result")
}

#[test]
fn a_model_the_provider_does_not_serve_falls_back_to_the_run_s_model() {
    agent_engine::providers::install_testing();
    let rig = Rig::new(PermissionMode::AcceptEdits);
    let input = json!({"description": "Look", "prompt": "Say hi", "model": "some-other-vendor-model"});
    let (model, history) = delegate(&rig, input, 2);
    assert_eq!(model.asked()[1].model, "m", "the sub-agent ran on the run's model");
    let (failed, content) = agent_result(&history);
    assert!(!failed, "{content}");
}

#[test]
fn a_worktree_outside_a_repository_with_a_commit_says_to_call_again_without_isolation() {
    let rig = Rig::new(PermissionMode::AcceptEdits);
    let input = json!({"description": "Look", "prompt": "Say hi", "isolation": "worktree"});
    let (_, history) = delegate(&rig, input, 1);
    let (failed, content) = agent_result(&history);
    assert!(failed && content.contains("without isolation"), "{content}");
}
