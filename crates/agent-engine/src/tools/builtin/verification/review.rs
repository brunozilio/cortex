use domain::{AgentStatus, EngineEvent, Verdict};

use super::{KIND, TOOLS, brief};
use crate::agent::{Agent, Ending, Review};
use crate::model::TurnRequest;
use crate::tools::Verification;
use crate::tools::builtin::shell::shells;
use crate::transcript::{Message, Role};

const JUDGE_TURNS: usize = 50;

pub fn reviewers() -> Vec<Box<dyn Review>> {
    vec![Box::new(Judge)]
}

struct Judge;

impl Review for Judge {
    fn review(&self, agent: &Agent<'_>, run: &[Message], attempts: &mut u8) -> Result<Option<Message>, Ending> {
        let Some(verification) = agent.workspace.verification.as_ref().filter(|_| agent.depth == 0) else {
            return Ok(None);
        };
        if !run.iter().any(|message| message.tool_uses().next().is_some()) {
            return Ok(None);
        }
        *attempts += 1;
        let client = agent.workspace.credentials.client;
        let before: Vec<String> = shells::list(client).into_iter().map(|(id, _, _)| id).collect();
        let verdict = verify(agent, verification, run, *attempts);
        for (id, _, running) in shells::list(client) {
            if running && !before.contains(&id) {
                shells::kill(client, &id);
            }
        }
        (agent.emit)(EngineEvent::Verified(verdict.clone()));
        if agent.control.is_stopped() {
            return Err(Ending::Cancelled);
        }
        Ok(verdict.should_retry().then(|| Message::user(verdict.feedback())))
    }
}

fn verify(agent: &Agent<'_>, verification: &Verification, run: &[Message], attempt: u8) -> Verdict {
    let specs = agent.request.tools.iter().filter(|spec| TOOLS.contains(&spec.name.as_str())).cloned();
    let control = agent.control.child_configured(None, true);
    let mut workspace = agent.workspace.fork(control.shared_stop());
    workspace.pre_approved = verification.allow.clone();
    workspace.hooks = Default::default();
    workspace.agent_name = KIND.into();
    if verification.model != agent.request.model {
        workspace.model.id = verification.model.clone();
        workspace.model.window = None;
        workspace.model.capabilities = None;
    }
    let index = agent.control.next_agent();
    (agent.emit)(EngineEvent::AgentSpawned {
        index,
        kind: KIND.into(),
        summary: format!("Verifying the work (attempt {attempt})"),
        prompt: verification.criteria.text.clone(),
    });
    (agent.emit)(EngineEvent::AgentUpdate { index, status: AgentStatus::Running { progress: 0.0 }, summary: None });
    let relay = |event: EngineEvent| match event {
        EngineEvent::TextDelta(_)
        | EngineEvent::ThinkingDelta(_)
        | EngineEvent::ToolStarted(_)
        | EngineEvent::ToolFinished { .. }
        | EngineEvent::ToolResult { .. } => (agent.emit)(EngineEvent::AgentEvent { index, event: Box::new(event) }),
        EngineEvent::Asked(_) => (agent.emit)(event),
        _ => {}
    };
    let started = std::time::Instant::now();
    let checker = Agent {
        api: agent.api,
        tools: agent.tools,
        workspace: &workspace,
        control: &control,
        emit: &relay,
        request: TurnRequest {
            model: verification.model.clone(),
            system: format!("{}\n\n{}", agent.request.system, crate::prompt::text("judge")),
            tools: specs.collect(),
            ..agent.request.clone()
        },
        agents: &[],
        depth: agent.depth + 1,
        turn_limit: JUDGE_TURNS,
    };
    let mut history = vec![Message::user(brief(&verification.criteria, run))];
    let verdict = match checker.run(&mut history) {
        Ok(_) => {
            let reply = history.iter().rev().find(|m| m.role == Role::Assistant).map(Message::text);
            Verdict::parse(&reply.unwrap_or_default(), attempt).unwrap_or_else(|e| Verdict::unavailable(e, attempt))
        }
        Err(ending) => Verdict::unavailable(format!("the judge stopped: {ending:?}"), attempt),
    };
    let seconds = u32::try_from(started.elapsed().as_secs()).unwrap_or(u32::MAX);
    let status = if verdict.passed { AgentStatus::Done { seconds } } else { AgentStatus::Failed };
    (agent.emit)(EngineEvent::AgentUpdate { index, status, summary: Some(format!("score {:.2}", verdict.score)) });
    verdict
}
