use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use platform::sync::Locked;

use crate::agent::Agent;
use crate::tools::Outcome;
use crate::transcript::Message;

pub(in crate::agent) struct Kept {
    pub(in crate::agent) input: Value,
    pub(in crate::agent) index: usize,
    pub(in crate::agent) history: Vec<Message>,
}

pub(in crate::agent) struct Resume {
    pub(in crate::agent) index: usize,
    pub(in crate::agent) history: Vec<Message>,
    pub(in crate::agent) message: String,
}

#[derive(Default, Clone)]
pub(in crate::agent) struct Conversations(Arc<Mutex<BTreeMap<String, Kept>>>);

impl Conversations {
    pub(in crate::agent) fn id(index: usize) -> String {
        format!("agent-{}", index + 1)
    }

    pub(in crate::agent) fn keep(&self, kept: Kept) -> String {
        let id = Self::id(kept.index);
        self.0.locked().insert(id.clone(), kept);
        id
    }

    pub(in crate::agent) fn has(&self, to: &str) -> bool {
        self.0.locked().contains_key(to)
    }

    fn take(&self, to: &str) -> Option<Kept> {
        self.0.locked().remove(to)
    }
}

impl Agent<'_> {
    pub(in crate::agent) fn send_message(&self, input: &Value, conversations: &Conversations) -> Outcome {
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
    pub(in crate::agent) fn to_session(&self, input: &Value, conversations: &Conversations) -> bool {
        let to = input["to"].as_str().map(str::trim).unwrap_or_default();
        self.depth == 0 && !conversations.has(to) && crate::tools::peers::open().iter().any(|peer| peer.name == to)
    }
}
