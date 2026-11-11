//! Behavioral coverage of custom-agent runtime features with real local tools.

mod configured;
mod hooks;
mod runtime;
mod verification;

use std::sync::Arc;

use agent_engine::{
    agent::Agent,
    control::Control,
    model::{TurnRequest, scripted::ScriptedModel},
    prompt,
    tools::{
        self, Workspace,
        subagent::{AgentTool, FoundAgent},
        subagent_config::AgentConfig,
    },
    transcript::{Block, Message},
};
use domain::{Effort, EngineEvent, PermissionMode};
fn call(id: &str, name: &str, input: serde_json::Value) -> Block {
    Block::ToolUse { id: id.into(), name: name.into(), input }
}
fn run(config: serde_json::Value, model: &ScriptedModel, workspace: &Workspace, mode: PermissionMode) -> Vec<Message> {
    let found = FoundAgent {
        name: "reviewer".into(),
        description: "Review".into(),
        instructions: "Do the review.".into(),
        config: AgentConfig::parse(&config).expect("config"),
    };
    let mut tools = tools::standard();
    tools.push(Box::new(AgentTool { agents: vec![found.clone()] }));
    let control = Control::new(mode, Arc::default());
    let emit = |event| assert!(!matches!(event, EngineEvent::Asked(_)), "unexpected interactive ask");
    let agent = Agent {
        api: model,
        tools: &tools,
        workspace,
        control: &control,
        emit: &emit,
        request: TurnRequest {
            model: "claude-opus-5-5".into(),
            system: workspace.prompt_context.as_ref().map(prompt::system).unwrap_or("Parent context".into()),
            messages: vec![],
            tools: Agent::specs(&tools),
            effort: Effort::Medium,
            max_tokens: 8192,
        },
        agents: &[found],
        depth: 0,
        turn_limit: 20,
    };
    let mut history = vec![Message::user("review")];
    agent.run(&mut history).expect("run completes");
    history
}
fn delegated() -> Result<agent_engine::model::Reply, agent_engine::model::ModelError> {
    ScriptedModel::reply(vec![call(
        "delegate",
        "Agent",
        serde_json::json!({"description":"Review project","prompt":"review","subagent_type":"reviewer"}),
    )])
}
fn done(text: &str) -> Result<agent_engine::model::Reply, agent_engine::model::ModelError> {
    ScriptedModel::reply(vec![Block::Text(text.into())])
}
