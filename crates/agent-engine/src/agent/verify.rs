use domain::{AgentStatus, EngineEvent, Verdict};

use super::{Agent, Ending};
use crate::judge::{self, Verification};
use crate::model::TurnRequest;
use crate::transcript::{Message, Role};

const JUDGE_TURNS: usize = 50;

impl Agent<'_> {
    pub(super) fn judged(&self, run: &[Message], attempts: &mut u8) -> Result<Option<Message>, Ending> {
        let Some(verification) = self.workspace.verification.as_ref().filter(|_| self.depth == 0) else {
            return Ok(None);
        };
        *attempts += 1;
        let verdict = self.verify(verification, run, *attempts);
        (self.emit)(EngineEvent::Verified(verdict.clone()));
        if self.control.is_stopped() {
            return Err(Ending::Cancelled);
        }
        Ok(verdict.should_retry().then(|| Message::user(verdict.feedback())))
    }

    fn verify(&self, verification: &Verification, run: &[Message], attempt: u8) -> Verdict {
        let specs = self.request.tools.iter().filter(|spec| judge::TOOLS.contains(&spec.name.as_str())).cloned();
        let control = self.control.child_configured(None, false);
        let mut workspace = self.workspace.fork(control.shared_stop());
        workspace.hooks = Default::default();
        workspace.agent_name = judge::KIND.into();
        if verification.model != self.request.model {
            workspace.model.id = verification.model.clone();
            workspace.model.window = None;
            workspace.model.capabilities = None;
        }
        let index = self.control.next_agent();
        (self.emit)(EngineEvent::AgentSpawned {
            index,
            kind: judge::KIND.into(),
            summary: format!("Verifying the work (attempt {attempt})"),
            prompt: verification.criteria.text.clone(),
        });
        (self.emit)(EngineEvent::AgentUpdate { index, status: AgentStatus::Running { progress: 0.0 }, summary: None });
        let relay = |event: EngineEvent| match event {
            EngineEvent::TextDelta(_)
            | EngineEvent::ThinkingDelta(_)
            | EngineEvent::ToolStarted(_)
            | EngineEvent::ToolFinished { .. }
            | EngineEvent::ToolResult { .. } => (self.emit)(EngineEvent::AgentEvent { index, event: Box::new(event) }),
            EngineEvent::Asked(_) => (self.emit)(event),
            _ => {}
        };
        let started = std::time::Instant::now();
        let checker = Agent {
            api: self.api,
            tools: self.tools,
            workspace: &workspace,
            control: &control,
            emit: &relay,
            request: TurnRequest {
                model: verification.model.clone(),
                system: format!("{}\n\n{}", self.request.system, crate::prompt::text("judge")),
                tools: specs.collect(),
                ..self.request.clone()
            },
            agents: &[],
            depth: self.depth + 1,
            turn_limit: JUDGE_TURNS,
        };
        let mut history = vec![Message::user(judge::brief(&verification.criteria, run))];
        let verdict = match checker.run(&mut history) {
            Ok(_) => {
                let reply = history.iter().rev().find(|m| m.role == Role::Assistant).map(Message::text);
                Verdict::parse(&reply.unwrap_or_default(), attempt).unwrap_or_else(|e| Verdict::unavailable(e, attempt))
            }
            Err(ending) => Verdict::unavailable(format!("the judge stopped: {ending:?}"), attempt),
        };
        let seconds = u32::try_from(started.elapsed().as_secs()).unwrap_or(u32::MAX);
        let status = if verdict.passed { AgentStatus::Done { seconds } } else { AgentStatus::Failed };
        (self.emit)(EngineEvent::AgentUpdate { index, status, summary: Some(format!("score {:.2}", verdict.score)) });
        verdict
    }
}
