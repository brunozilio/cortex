use domain::EngineEvent;
use domain::trace::{ModelCall, RunInfo};
use serde::{Deserialize, Serialize};

use crate::transcript::Message;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Entry {
    Message(Message),
    Truncated {
        keep: usize,
    },
    Restarted,
    Event {
        at_ms: u64,
        event: EngineEvent,
    },
    Prompted {
        at_ms: u64,
        prompt: String,
    },
    Called {
        at_ms: u64,
        #[serde(default)]
        agent: Option<usize>,
        call: ModelCall,
    },
    Opened {
        at_ms: u64,
        run: RunInfo,
    },
}

const CONVERSATION: [&str; 3] = ["message", "truncated", "restarted"];

#[derive(Deserialize)]
struct Kind {
    kind: String,
}

pub(super) fn changes(kept: &[Message], history: &[Message]) -> Vec<Entry> {
    if history.starts_with(kept) {
        return history[kept.len()..].iter().cloned().map(Entry::Message).collect();
    }
    if kept.starts_with(history) {
        return vec![Entry::Truncated { keep: history.len() }];
    }
    std::iter::once(Entry::Restarted).chain(history.iter().cloned().map(Entry::Message)).collect()
}

pub(super) fn conversation(entries: impl IntoIterator<Item = Entry>) -> Vec<Message> {
    let mut history = Vec::new();
    for entry in entries {
        match entry {
            Entry::Message(message) => history.push(message),
            Entry::Truncated { keep } => history.truncate(keep),
            Entry::Restarted => history.clear(),
            Entry::Event { .. } | Entry::Prompted { .. } | Entry::Called { .. } | Entry::Opened { .. } => {}
        }
    }
    history
}

pub(super) fn resume(text: &str) -> Result<Vec<Message>, String> {
    let lines: Vec<&str> = text.lines().filter(|line| !line.trim().is_empty()).collect();
    let mut since_restart = Vec::new();
    for line in lines.iter().rev() {
        let kind = serde_json::from_str::<Kind>(line).map_err(invalid)?.kind;
        let restarted = kind == "restarted";
        if CONVERSATION.contains(&kind.as_str()) {
            since_restart.push(*line);
        }
        if restarted {
            break;
        }
    }
    let entries: Result<Vec<Entry>, String> =
        since_restart.into_iter().rev().map(|line| serde_json::from_str(line).map_err(invalid)).collect();
    Ok(conversation(entries?))
}

pub(super) fn all(text: &str) -> Result<Vec<Entry>, String> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).map_err(invalid))
        .collect()
}

fn invalid(error: serde_json::Error) -> String {
    format!("the saved conversation is invalid: {error}")
}
