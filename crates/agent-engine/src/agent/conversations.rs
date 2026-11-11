use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use super::Agent;
use crate::tools::Outcome;
use crate::transcript::Message;

pub(super) struct Kept {
    pub(super) input: Value,
    pub(super) index: usize,
    pub(super) history: Vec<Message>,
}

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

    pub(super) fn keep(&self, kept: Kept) -> String {
        let id = Self::id(kept.index);
        self.0.lock().expect("conversations").insert(id.clone(), kept);
        id
    }

    pub(super) fn has(&self, to: &str) -> bool {
        self.0.lock().expect("conversations").contains_key(to)
    }

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
            conversations.keep(kept);
        }
        outcome
    }
}

impl Agent<'_> {
    pub(super) fn to_session(&self, input: &Value, conversations: &Conversations) -> bool {
        let to = input["to"].as_str().map(str::trim).unwrap_or_default();
        self.depth == 0
            && !conversations.has(to)
            && crate::tools::peers::of_owner(self.workspace.credentials.owner.as_ref())
                .iter()
                .any(|peer| peer.name == to)
    }
}
