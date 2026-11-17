use std::sync::atomic::{AtomicUsize, Ordering};

use agent_engine::agent::Agent;
use agent_engine::model::scripted::ScriptedModel;
use agent_engine::model::{BoxFuture, CancellationToken, ModelApi, ModelError, OnDelta, Reply, TurnRequest};
use agent_engine::tools;
use agent_engine::transcript::{Block, Message};
use domain::PermissionMode;
use serde_json::json;

use crate::{Rig, request, tool_use};

struct LongReport {
    parent: AtomicUsize,
    handed_over: std::sync::Mutex<String>,
}

impl ModelApi for LongReport {
    fn turn<'a>(
        &'a self,
        request: &'a TurnRequest,
        _: OnDelta<'a>,
        _: &'a CancellationToken,
    ) -> BoxFuture<'a, Result<Reply, ModelError>> {
        Box::pin(std::future::ready(self.answer(request)))
    }
}

impl LongReport {
    fn answer(&self, request: &TurnRequest) -> Result<Reply, ModelError> {
        if request.system.contains("# You are a sub-agent") {
            return ScriptedModel::reply(vec![Block::Text(format!("REPORT-START {} REPORT-END", "x".repeat(60_000)))]);
        }
        match self.parent.fetch_add(1, Ordering::SeqCst) {
            0 => ScriptedModel::reply(vec![tool_use(
                "a",
                "Agent",
                json!({"description":"Long review","prompt":"review","run_in_background":true}),
            )]),
            1 => ScriptedModel::reply(vec![Block::Text("Waiting for the review.".into())]),
            _ => {
                let last = request.messages.last().map(Message::text).unwrap_or_default();
                *self.handed_over.lock().expect("handed over") = last;
                ScriptedModel::reply(vec![Block::Text("Reviewed".into())])
            }
        }
    }
}

#[test]
fn a_long_background_report_is_saved_whole_and_handed_over_by_its_ends() {
    let rig = Rig::new(PermissionMode::Bypass);
    let model = LongReport { parent: AtomicUsize::new(0), handed_over: std::sync::Mutex::default() };
    let mut tools = tools::standard();
    tools.push(Box::new(tools::subagent::AgentTool::default()));
    tools.extend(tools::task::tools());
    let emit = |_| {};
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
    agent.run(&mut vec![Message::user("review")]).expect("run");
    let handed_over = model.handed_over.lock().expect("handed over").clone();
    assert!(handed_over.starts_with("Background agents completed."), "{}", &handed_over[..80.min(handed_over.len())]);
    assert!(handed_over.contains("REPORT-START") && handed_over.contains("REPORT-END"));
    assert!(handed_over.contains("Full output saved to: "));
    assert!(handed_over.len() < 8_000, "{}", handed_over.len());
}
