use super::Ending;
use crate::model::{ModelError, Reply};
use crate::transcript::{Block, Message, StopReason};

const MAX_CONTINUATIONS: usize = 3;

#[derive(Debug, Default)]
pub(super) struct Cutoffs {
    retried_empty: bool,
    continuations: usize,
}

impl Cutoffs {
    pub(super) fn ask_again_for_empty(&mut self, reply: &Reply, has_calls: bool) -> bool {
        let empty = !has_calls
            && reply.stop != StopReason::MaxTokens
            && reply.message.text().trim().is_empty()
            && !reply.message.blocks.iter().any(|b| matches!(b, Block::Image(_)));
        if empty && !self.retried_empty {
            self.retried_empty = true;
            return true;
        }
        false
    }

    pub(super) fn answered(&mut self) {
        self.continuations = 0;
    }

    pub(super) fn continuation(&mut self, cut: &Message) -> Result<Message, Ending> {
        self.continuations += 1;
        if self.continuations > MAX_CONTINUATIONS {
            return Err(Ending::Failed(ModelError::Provider(format!(
                "The reply hit the output limit {} times in a row, so the run stopped.",
                self.continuations
            ))));
        }
        Ok(Message::user(if cut.text().trim().is_empty() {
            "Your output reached its token limit while you were still thinking, before any answer. Reason much more briefly and get to the answer or the next tool call."
        } else {
            "Your output reached its token limit. Continue exactly at the cutoff, with no apology and no recap. New text is appended to the same response, so preserve code fences and machine-readable syntax. If much remains, break the rest of the work into smaller pieces."
        }))
    }
}
