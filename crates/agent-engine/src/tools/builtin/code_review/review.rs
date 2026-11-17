use domain::review::{already_reviewed, feedback, parse};
use domain::{AgentStatus, EngineEvent};

use super::{KIND, TOOLS, brief, changed};
use crate::agent::{Agent, Ending, Review};
use crate::model::TurnRequest;
use crate::transcript::{Message, Role};

const REVIEW_TURNS: usize = 30;

pub fn reviewers() -> Vec<Box<dyn Review>> {
    vec![Box::new(CodeReview)]
}

struct CodeReview;

impl Review for CodeReview {
    fn review(&self, agent: &Agent<'_>, run: &[Message], _attempts: &mut u8) -> Result<Option<Message>, Ending> {
        if agent.depth != 0 || !agent.workspace.policy.auto_review {
            return Ok(None);
        }
        let mut paths = changed(run);
        paths.retain(|path| agent.workspace.resolve(path).starts_with(&agent.workspace.cwd));
        let texts = run.iter().filter(|message| message.role == Role::User).map(Message::text);
        if paths.is_empty() || already_reviewed(texts) {
            return Ok(None);
        }
        let findings = reviewed(agent, &brief(run, &paths));
        if agent.control.is_stopped() {
            return Err(Ending::Cancelled);
        }
        let findings = match findings {
            Ok(findings) => findings,
            Err(error) => {
                tracing::warn!(%error, "the code review gave no findings");
                return Ok(None);
            }
        };
        (agent.emit)(EngineEvent::Reviewed(findings.clone()));
        Ok(feedback(&findings).map(Message::user))
    }
}

fn reviewed(agent: &Agent<'_>, brief: &str) -> Result<Vec<domain::review::Finding>, String> {
    let specs = agent.request.tools.iter().filter(|spec| TOOLS.contains(&spec.name.as_str())).cloned();
    let control = agent.control.child_configured(None, true);
    let mut workspace = agent.workspace.fork(control.shared_stop());
    workspace.hooks = Default::default();
    workspace.agent_name = KIND.into();
    let index = agent.control.next_agent();
    (agent.emit)(EngineEvent::AgentSpawned {
        index,
        kind: KIND.into(),
        summary: "Reviewing the changes".into(),
        prompt: brief.to_owned(),
    });
    (agent.emit)(EngineEvent::AgentUpdate { index, status: AgentStatus::Running { progress: 0.0 }, summary: None });
    let relay = |event: EngineEvent| match event {
        EngineEvent::ToolStarted(_) | EngineEvent::ToolFinished { .. } | EngineEvent::ToolResult { .. } => {
            (agent.emit)(EngineEvent::AgentEvent { index, event: Box::new(event) });
        }
        _ => {}
    };
    let started = std::time::Instant::now();
    let reviewer = Agent {
        api: agent.api,
        tools: agent.tools,
        workspace: &workspace,
        control: &control,
        emit: &relay,
        request: TurnRequest {
            system: format!("{}\n\n{}", agent.request.system, crate::prompt::text("reviewer")),
            tools: specs.collect(),
            ..agent.request.clone()
        },
        agents: &[],
        depth: agent.depth + 1,
        turn_limit: REVIEW_TURNS,
    };
    let mut history = vec![Message::user(brief)];
    let found = match crate::agent::current::within(index, || reviewer.run(&mut history)) {
        Ok(_) => {
            let reply = history.iter().rev().find(|message| message.role == Role::Assistant).map(Message::text);
            parse(&reply.unwrap_or_default())
        }
        Err(ending) => Err(format!("the reviewer stopped: {ending:?}")),
    };
    let seconds = u32::try_from(started.elapsed().as_secs()).unwrap_or(u32::MAX);
    let summary = found.as_ref().map_or_else(Clone::clone, |findings| format!("{} findings", findings.len()));
    let status = if found.is_ok() { AgentStatus::Done { seconds } } else { AgentStatus::Failed };
    (agent.emit)(EngineEvent::AgentUpdate { index, status, summary: Some(summary) });
    found
}
