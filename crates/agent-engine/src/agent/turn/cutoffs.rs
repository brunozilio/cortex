use super::interception::Intercepted;
use super::{Call, Exchange, Next, TurnState};
use crate::agent::permission::egress;
use crate::agent::{Agent, Ending};
use crate::model::{ModelError, Reply};
use crate::transcript::{Block, Message, StopReason, replied, told};

impl Agent<'_> {
    pub(super) fn take_reply(&self, turn: &mut TurnState<'_>, exchange: Exchange) -> Result<Next, Ending> {
        let Exchange { request, reply } = exchange;
        let reply = match self.intercept_reply(reply, &request, &mut turn.rejections)? {
            Intercepted::Accepted(reply) => reply,
            Intercepted::Rejected => return Ok(Next::AskModel),
        };
        let calls: Vec<Call> = reply
            .message
            .tool_uses()
            .map(|(id, name, input)| (id.to_owned(), name.to_owned(), input.clone()))
            .collect();
        if turn.cutoffs.ask_again_for_empty(&reply, !calls.is_empty()) {
            return Ok(Next::AskModel);
        }
        turn.reminders.observe(&calls);
        if let Some(safety) = &self.workspace.policy.safety {
            safety.observe(turn.history);
        }
        self.workspace.tracking.hear(egress::heard(turn.history));
        let said = replied(turn.history, reply.message);
        turn.counted.observe(reply.usage, turn.history.len());
        if reply.stop != StopReason::MaxTokens {
            turn.cutoffs.answered();
        }
        after_the_reply(turn, reply.stop, calls, said)
    }
}

pub(super) fn after_the_reply(
    turn: &mut TurnState<'_>,
    stop: StopReason,
    calls: Vec<Call>,
    said: bool,
) -> Result<Next, Ending> {
    match stop {
        StopReason::MaxTokens if calls.is_empty() => {
            let continuation = turn.cutoffs.continuation(turn.history.last().filter(|_| said))?;
            told(turn.history, continuation);
            Ok(Next::AskModel)
        }
        StopReason::Refusal => Err(Ending::Refused),
        StopReason::PauseTurn if calls.is_empty() => Ok(Next::AskModel),
        _ if calls.is_empty() => Ok(Next::Finishing { said }),
        _ => Ok(Next::RunTools(calls)),
    }
}

const MAX_CONTINUATIONS: usize = 3;
const MAX_STRUCTURED_NUDGES: usize = 3;
const MAX_STOP_HOOK_BLOCKS: usize = 3;

pub const INTERRUPTED_BY_THE_USER: &str = "\n\n[interrupted by the user]";

pub(super) fn streamed(so_far: &mut String, text: String) -> String {
    so_far.push_str(&text);
    text
}

pub(super) fn interrupted(history: &mut Vec<Message>, shown: &str) -> Ending {
    if !shown.trim().is_empty() {
        let said = vec![Block::Text(shown.to_owned()), Block::Text(INTERRUPTED_BY_THE_USER.into())];
        history.push(Message::assistant(said));
    }
    Ending::Cancelled
}

#[derive(Debug, Default)]
pub(super) struct Cutoffs {
    retried_empty: bool,
    continuations: usize,
    structured_nudges: usize,
    stop_hook_blocks: usize,
}

impl Cutoffs {
    pub(super) fn structured_nudge(&mut self) -> Result<Message, Ending> {
        if self.structured_nudges >= MAX_STRUCTURED_NUDGES {
            return Err(Ending::Failed(ModelError::Provider(format!(
                "The model was asked {MAX_STRUCTURED_NUDGES} times to call StructuredOutput and never did, so the run stopped without a structured result."
            ))));
        }
        self.structured_nudges += 1;
        Ok(Message::user(
            "The configured response contract requires calling StructuredOutput with a valid result before completing. Use the tool now.",
        ))
    }

    pub(super) fn stop_blocked(&mut self, event: &str, reason: &str, context: &str) -> Result<Message, String> {
        if self.stop_hook_blocks >= MAX_STOP_HOOK_BLOCKS {
            return Err(format!(
                "The {event} hook blocked completion {MAX_STOP_HOOK_BLOCKS} times in a row and blocked it again, so the turn ended anyway. Its reason: {reason}"
            ));
        }
        self.stop_hook_blocks += 1;
        let said = format!("Completion blocked by hook: {reason}\n{context}");
        Ok(Message::user(said.trim_end()))
    }

    pub(super) fn stop_allowed(&mut self) {
        self.stop_hook_blocks = 0;
    }

    pub(super) fn ask_again_for_empty(&mut self, reply: &Reply, has_calls: bool) -> bool {
        let empty = !has_calls
            && !matches!(reply.stop, StopReason::MaxTokens | StopReason::PauseTurn)
            && reply.message.text().trim().is_empty()
            && !reply.message.blocks.iter().any(|b| matches!(b, Block::Image(_) | Block::Native { .. }));
        if empty && !self.retried_empty {
            self.retried_empty = true;
            return true;
        }
        false
    }

    pub(super) fn answered(&mut self) {
        self.continuations = 0;
    }

    pub(super) fn continuation(&mut self, cut: Option<&Message>) -> Result<Message, Ending> {
        self.continuations += 1;
        if self.continuations > MAX_CONTINUATIONS {
            return Err(Ending::Failed(ModelError::Provider(format!(
                "The reply hit the output limit {} times in a row, so the run stopped.",
                self.continuations
            ))));
        }
        Ok(Message::user(if cut.is_none_or(|cut| cut.text().trim().is_empty()) {
            "Your output reached its token limit while you were still thinking, before any answer. Reason much more briefly and get to the answer or the next tool call."
        } else {
            "Your output reached its token limit. Continue exactly at the cutoff, with no apology and no recap. New text is appended to the same response, so preserve code fences and machine-readable syntax. If much remains, break the rest of the work into smaller pieces."
        }))
    }
}
