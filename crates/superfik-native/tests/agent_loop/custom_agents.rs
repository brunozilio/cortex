//! Custom agents: their tools, permission mode, model, effort and turns.

use serde_json::json;
use superfik_core::{Effort, PermissionMode};
use superfik_native::agent::Agent;
use superfik_native::model::TurnRequest;
use superfik_native::model::scripted::ScriptedModel;
use superfik_native::tools;
use superfik_native::transcript::{Block, Message};

use crate::{Rig, request, tool_use};

#[test]
fn configured_agent_filters_tools_enforces_runtime_denial_and_selects_model_effort() {
    use superfik_native::tools::{
        subagent::{AgentTool, FoundAgent},
        subagent_config::AgentConfig,
    };
    let rig = Rig::new(PermissionMode::Bypass);
    let found = FoundAgent {
        name: "reviewer".into(),
        description: "Read-only review".into(),
        instructions: "Review only.".into(),
        config: AgentConfig::parse(
            &json!({"tools":["Read"],"permissionMode":"plan","model":"haiku","effort":"low","maxTurns":2}),
        )
        .expect("config"),
    };
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![tool_use(
            "delegate",
            "Agent",
            json!({"description":"Read-only check","prompt":"review","subagent_type":"reviewer"}),
        )]),
        ScriptedModel::reply(vec![tool_use("bad", "Bash", json!({"command":"echo bad > forbidden"}))]),
        ScriptedModel::reply(vec![Block::Text("Bash was unavailable.".into())]),
        ScriptedModel::reply(vec![Block::Text("Reviewed.".into())]),
    ]);
    let mut tools = tools::standard();
    tools.push(Box::new(AgentTool { agents: vec![found.clone()] }));
    let emit = |_| {};
    let agent = Agent {
        api: &model,
        tools: &tools,
        workspace: &rig.workspace,
        control: &rig.control,
        emit: &emit,
        request: TurnRequest { tools: Agent::specs(&tools), ..request() },
        agents: &[found],
        depth: 0,
        turn_limit: 200,
    };
    agent.run(&mut vec![Message::user("review")]).expect("run");
    assert!(!rig.dir.path().join("forbidden").exists());
    let calls = model.asked();
    assert_eq!(calls[1].model, "claude-haiku-5-5");
    assert_eq!(calls[1].effort, Effort::Low);
    assert_eq!(calls[1].tools.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(), ["Read"]);
    assert!(
        matches!(&calls[2].messages.last().expect("feedback").blocks[0],Block::ToolResult{is_error:true,content,..} if content.contains("not available"))
    );
}
#[test]
fn custom_agent_turn_limit_stops_child_and_parent_can_continue() {
    use superfik_native::tools::{
        subagent::{AgentTool, FoundAgent},
        subagent_config::AgentConfig,
    };
    let rig = Rig::new(PermissionMode::Bypass);
    let found = FoundAgent {
        name: "limited".into(),
        description: "One model turn".into(),
        instructions: String::new(),
        config: AgentConfig::parse(&json!({"maxTurns":1})).expect("config"),
    };
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![tool_use(
            "delegate",
            "Agent",
            json!({"description":"Limited check","prompt":"read","subagent_type":"limited"}),
        )]),
        ScriptedModel::reply(vec![tool_use("read", "Read", json!({"file_path":"notes.txt"}))]),
        ScriptedModel::reply(vec![Block::Text("Parent continued.".into())]),
    ]);
    let mut tools = tools::standard();
    tools.push(Box::new(AgentTool { agents: vec![found.clone()] }));
    let emit = |_| {};
    let agent = Agent {
        api: &model,
        tools: &tools,
        workspace: &rig.workspace,
        control: &rig.control,
        emit: &emit,
        request: TurnRequest { tools: Agent::specs(&tools), ..request() },
        agents: &[found],
        depth: 0,
        turn_limit: 200,
    };
    let mut history = vec![Message::user("check")];
    agent.run(&mut history).expect("parent completes");
    assert_eq!(model.asked().len(), 3);
    assert!(
        matches!(&history[2].blocks[0],Block::ToolResult{is_error:true,content,..} if content.contains("TooManyTurns"))
    );
    assert_eq!(history.last().expect("reply").text(), "Parent continued.");
}
