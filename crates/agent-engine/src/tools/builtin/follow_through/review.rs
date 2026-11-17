use domain::follow_through::{Judgement, nudges, worth_judging};

use crate::agent::{Agent, Ending, Review};
use crate::model::{BlockingTurn, TurnRequest};
use crate::transcript::{Message, Role};

const JUDGE_TOKENS: u32 = 300;

pub fn reviewers() -> Vec<Box<dyn Review>> {
    vec![Box::new(FollowThrough)]
}

struct FollowThrough;

impl Review for FollowThrough {
    fn review(&self, agent: &Agent<'_>, run: &[Message], _attempts: &mut u8) -> Result<Option<Message>, Ending> {
        if agent.depth != 0 || !agent.workspace.policy.follow_through {
            return Ok(None);
        }
        if !run.iter().any(|message| message.tool_uses().next().is_some()) {
            return Ok(None);
        }
        let answer = run.iter().rev().find(|message| message.role == Role::Assistant).map(Message::text);
        let answer = answer.unwrap_or_default();
        let users: Vec<String> = run.iter().filter(|message| message.role == Role::User).map(Message::text).collect();
        if !worth_judging(&answer, nudges(users.iter().map(String::as_str))) {
            return Ok(None);
        }
        let request = users.first().map(String::as_str).unwrap_or_default();
        let judged = agent
            .api
            .turn_blocking(&judge_request(agent, request, &answer), &mut |_| {}, agent.control.token())
            .map(|reply| Judgement::parse(&reply.message.text()));
        if agent.control.is_stopped() {
            return Err(Ending::Cancelled);
        }
        match judged {
            Ok(Some(judgement)) => Ok(judgement.nudge().map(Message::user)),
            Ok(None) => Ok(None),
            Err(error) => {
                tracing::warn!(%error, "the follow-through check could not run");
                Ok(None)
            }
        }
    }
}

fn judge_request(agent: &Agent<'_>, request: &str, answer: &str) -> TurnRequest {
    let small = agent.workspace.models.small_model.as_deref();
    let template =
        crate::tools::builtin::web::guarded::reader(small, &agent.request, agent.workspace.models.model.provider);
    TurnRequest {
        system: crate::prompt::text("follow-through"),
        messages: vec![Message::user(format!(
            "<request>\n{request}\n</request>\n\n<last_answer>\n{answer}\n</last_answer>"
        ))],
        effort: domain::Effort::Low,
        max_tokens: template.max_tokens.min(JUDGE_TOKENS),
        ..template
    }
}
