//! The sub-agents of one run that can take another message: each one's
//! conversation, kept after its report until the run ends, and the
//! `SendMessage` call that continues it.

use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use super::Agent;
use crate::tools::Outcome;
use crate::transcript::Message;

/// A finished sub-agent: how it was started, its card and its conversation.
pub(super) struct Kept {
    pub(super) input: Value,
    pub(super) index: usize,
    pub(super) history: Vec<Message>,
}

/// What a continued sub-agent picks up: its card, its conversation and the
/// new message.
pub(super) struct Resume {
    pub(super) index: usize,
    pub(super) history: Vec<Message>,
    pub(super) message: String,
}

#[derive(Default, Clone)]
pub(super) struct Conversations(Arc<Mutex<BTreeMap<String, Kept>>>);

impl Conversations {
    pub(super) fn id(index: usize) -> String {
        format!("agent-{}", index + 1)
    }

    /// Keeps a finished sub-agent; returns the id to message it by.
    pub(super) fn keep(&self, kept: Kept) -> String {
        let id = Self::id(kept.index);
        self.0.lock().expect("conversations").insert(id.clone(), kept);
        id
    }

    /// Whether `to` is a sub-agent kept here.
    pub(super) fn has(&self, to: &str) -> bool {
        self.0.lock().expect("conversations").contains_key(to)
    }

    /// Takes a sub-agent out while it works on a message; `keep` puts it back.
    fn take(&self, to: &str) -> Option<Kept> {
        self.0.lock().expect("conversations").remove(to)
    }
}

impl Agent<'_> {
    pub(super) fn send_message(&self, input: &Value, conversations: &Conversations) -> Outcome {
        if self.depth > 0 {
            return Outcome::error("A sub-agent cannot message other agents; do the work yourself.");
        }
        let Some(to) = input["to"].as_str().map(str::trim).filter(|to| !to.is_empty()) else {
            return Outcome::error("Missing required parameter `to`.");
        };
        let Some(message) = input["message"].as_str().filter(|m| !m.trim().is_empty()) else {
            return Outcome::error("Missing required parameter `message`.");
        };
        let Some(kept) = conversations.take(to) else {
            return Outcome::error(format!(
                "No agent or session {to}. Use the agent_id at the end of an Agent report (an agent still running \
                 in the background takes messages once collected with TaskOutput), or a name ListSessions gives."
            ));
        };
        let resume = Resume { index: kept.index, history: kept.history.clone(), message: message.to_owned() };
        let outcome = self.sub_agent_with(&kept.input, self.control, self.workspace, &[], conversations, Some(resume));
        if outcome.is_error {
            // It stopped partway: it goes on from before this message.
            conversations.keep(kept);
        }
        outcome
    }
}

impl Agent<'_> {
    /// Whether `SendMessage` with `input` goes to another of the user's
    /// sessions: no sub-agent of this run by that name, and a session is.
    pub(super) fn to_session(&self, input: &Value, conversations: &Conversations) -> bool {
        let to = input["to"].as_str().map(str::trim).unwrap_or_default();
        self.depth == 0
            && !conversations.has(to)
            && crate::tools::peers::of_owner(self.workspace.credentials.owner.as_ref())
                .iter()
                .any(|peer| peer.name == to)
    }
}
