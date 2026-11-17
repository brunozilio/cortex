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
    #[serde(skip)]
    Amended {
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
    Shown(domain::request::Shown),
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

impl Entry {
    pub fn is_conversation(&self) -> bool {
        matches!(self, Entry::Message(_) | Entry::Truncated { .. } | Entry::Amended { .. } | Entry::Restarted)
    }
}

const CONVERSATION: [&str; 3] = ["message", "truncated", "restarted"];
const KIND_FIRST: &str = "{\"kind\":\"";

#[derive(Deserialize)]
struct Head {
    kind: String,
    #[serde(default)]
    seq: Option<u64>,
    #[serde(default)]
    amended: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Line {
    pub(super) seq: Option<u64>,
    pub(super) entry: Entry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Holds {
    Conversation,
    TraceOnly,
}

pub(super) fn changes(kept: &[Message], history: &[Message]) -> Vec<Entry> {
    let same = kept.iter().zip(history).take_while(|(kept, said)| kept == said).count();
    let added = || history[same..].iter().cloned().map(Entry::Message);
    if same == kept.len() {
        return added().collect();
    }
    if same == history.len() {
        return vec![Entry::Truncated { keep: same }];
    }
    if same == 0 {
        return restarted(history);
    }
    std::iter::once(Entry::Amended { keep: same }).chain(added()).collect()
}

pub(super) fn restarted(history: &[Message]) -> Vec<Entry> {
    std::iter::once(Entry::Restarted).chain(history.iter().cloned().map(Entry::Message)).collect()
}

pub(super) fn conversation(entries: impl IntoIterator<Item = Entry>) -> Vec<Message> {
    let mut history = Vec::new();
    for entry in entries {
        match entry {
            Entry::Message(message) => history.push(message),
            Entry::Truncated { keep } | Entry::Amended { keep } => history.truncate(keep),
            Entry::Restarted => history.clear(),
            Entry::Event { .. }
            | Entry::Prompted { .. }
            | Entry::Shown(_)
            | Entry::Called { .. }
            | Entry::Opened { .. } => {}
        }
    }
    history
}

pub(super) fn encoded(entry: &Entry, seq: u64) -> Result<String, String> {
    let mut line = match entry {
        Entry::Amended { keep } => format!("{{\"kind\":\"truncated\",\"keep\":{keep},\"amended\":true}}"),
        other => serde_json::to_string(other).map_err(|error| error.to_string())?,
    };
    if line.pop() != Some('}') {
        return Err("a session entry is not a JSON object".into());
    }
    line.push_str(&format!(",\"seq\":{seq}}}\n"));
    Ok(line)
}

fn decoded(line: &str) -> Result<Line, serde_json::Error> {
    let head: Head = serde_json::from_str(line)?;
    let entry = match serde_json::from_str(line)? {
        Entry::Truncated { keep } if head.amended => Entry::Amended { keep },
        entry => entry,
    };
    Ok(Line { seq: head.seq, entry })
}

fn kind_at_the_start(line: &str) -> Option<&str> {
    let rest = line.trim_start().strip_prefix(KIND_FIRST)?;
    rest.split_once('"').map(|(kind, _)| kind)
}

fn is_trace_line(line: &str) -> bool {
    kind_at_the_start(line).is_some_and(|kind| !CONVERSATION.contains(&kind))
}

pub(super) fn written(text: &str) -> &str {
    if text.is_empty() || text.ends_with('\n') {
        return text;
    }
    let last = text.rfind('\n').map_or(0, |at| at + 1);
    if serde_json::from_str::<Head>(&text[last..]).is_ok() {
        return text;
    }
    tracing::warn!(bytes = text.len() - last, "dropping the last line of a saved conversation: it was cut mid-write");
    &text[..last]
}

#[derive(Debug, Default, PartialEq)]
pub(super) struct Resumed {
    pub(super) history: Vec<Message>,
    pub(super) last_seq: u64,
    pub(super) skipped: Vec<String>,
}

pub(super) fn resume(text: &str) -> Resumed {
    let lines: Vec<&str> = written(text).lines().filter(|line| !line.trim().is_empty()).collect();
    let mut since_restart = Vec::new();
    let mut skipped = Vec::new();
    let mut last_seq = 0;
    for line in lines.iter().rev() {
        let head = match serde_json::from_str::<Head>(line) {
            Ok(head) => head,
            Err(error) if is_trace_line(line) => {
                tracing::warn!(%error, "skipping a trace line that does not parse; the conversation is unaffected");
                continue;
            }
            Err(error) => {
                tracing::warn!(%error, "skipping a conversation line that does not parse");
                skipped.push((*line).to_owned());
                continue;
            }
        };
        last_seq = last_seq.max(head.seq.unwrap_or(0));
        if CONVERSATION.contains(&head.kind.as_str()) {
            since_restart.push(*line);
        }
        if head.kind == "restarted" {
            break;
        }
    }
    skipped.reverse();
    let mut entries = Vec::new();
    for line in since_restart.into_iter().rev() {
        match decoded(line) {
            Ok(line) => entries.push(line.entry),
            Err(error) => {
                tracing::warn!(%error, "skipping a conversation line that does not parse");
                skipped.push(line.to_owned());
            }
        }
    }
    Resumed { history: conversation(entries), last_seq, skipped }
}

pub(super) fn lines(text: &str, file: Holds) -> (Vec<Line>, Vec<String>) {
    let (mut read, mut skipped) = (Vec::new(), Vec::new());
    for line in written(text).lines().filter(|line| !line.trim().is_empty()) {
        match decoded(line) {
            Ok(line) => read.push(line),
            Err(error) if file == Holds::TraceOnly || is_trace_line(line) => {
                tracing::warn!(%error, "skipping a trace line that does not parse");
            }
            Err(error) => {
                tracing::warn!(%error, "skipping a conversation line that does not parse");
                skipped.push(line.to_owned());
            }
        }
    }
    (read, skipped)
}

pub(super) fn merged(conversation: Vec<Line>, trace: Vec<Line>) -> Vec<Entry> {
    let mut conversation = keyed(conversation).into_iter().peekable();
    let mut trace = keyed(trace).into_iter().peekable();
    let mut entries = Vec::new();
    loop {
        let from_trace = match (conversation.peek(), trace.peek()) {
            (Some((said, _)), Some((traced, _))) => traced < said,
            (None, Some(_)) => true,
            (Some(_), None) => false,
            (None, None) => return entries,
        };
        let next = if from_trace { trace.next() } else { conversation.next() };
        entries.extend(next.map(|(_, entry)| entry));
    }
}

fn keyed(lines: Vec<Line>) -> Vec<(u64, Entry)> {
    let mut last = 0;
    lines
        .into_iter()
        .map(|line| {
            last = last.max(line.seq.unwrap_or(last));
            (last, line.entry)
        })
        .collect()
}

#[cfg(test)]
#[path = "log_tests.rs"]
mod tests;
