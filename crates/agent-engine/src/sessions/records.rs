use domain::trace::Record;
use domain::{EngineEvent, ToolCall};

use super::Entry;
use crate::transcript::{Block, Message, Role};

pub fn records(entries: Vec<Entry>) -> Vec<Record> {
    let traced = entries
        .iter()
        .position(|entry| !matches!(entry, Entry::Message(_) | Entry::Restarted | Entry::Truncated { .. }));
    let legacy = traced.unwrap_or(entries.len());
    let mut records = Vec::new();
    if entries[..legacy].iter().any(|entry| matches!(entry, Entry::Message(_))) {
        records.push(Record::Partial);
    }
    for (at, entry) in entries.into_iter().enumerate() {
        match entry {
            Entry::Message(message) if at < legacy => records.extend(replayed(message)),
            Entry::Message(_) => {}
            Entry::Truncated { keep } => records.push(Record::Truncated { keep }),
            Entry::Restarted => records.push(Record::Restarted),
            Entry::Event { at_ms, event } => records.push(Record::Event { at_ms, event }),
            Entry::Prompted { at_ms, prompt } => records.push(Record::Prompted { at_ms, prompt }),
            Entry::Called { at_ms, agent, call } => records.push(Record::Called { at_ms, agent, call }),
            Entry::Opened { at_ms, run } => records.push(Record::Opened { at_ms, run }),
        }
    }
    records
}

fn replayed(message: Message) -> Vec<Record> {
    let event = |event: EngineEvent| Record::Event { at_ms: 0, event };
    let mut said = String::new();
    let mut records = Vec::new();
    for block in message.blocks {
        match (message.role, block) {
            (Role::User, Block::Text(text)) => said.push_str(&text),
            (Role::Assistant, Block::Text(text)) => records.push(event(EngineEvent::TextDelta(text))),
            (_, Block::Thinking { text, .. }) => records.push(event(EngineEvent::ThinkingDelta(text))),
            (_, Block::ToolUse { id, name, input }) => {
                records.push(event(EngineEvent::ToolStarted(ToolCall::new(id, name, "").with_input(input))))
            }
            (_, Block::ToolResult { id, content, is_error } | Block::ToolResultRich { id, content, is_error, .. }) => {
                records.push(event(EngineEvent::ToolResult { id, output: content, failed: is_error }))
            }
            _ => {}
        }
    }
    if !said.is_empty() {
        records.insert(0, Record::Prompted { at_ms: 0, prompt: said });
    }
    records
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn a_session_saved_before_traces_is_replayed_from_its_messages_and_marked_partial() {
        let entries = vec![
            Entry::Message(Message::user("fix the build")),
            Entry::Message(Message::assistant(vec![
                Block::Text("Looking.".into()),
                Block::ToolUse { id: "r".into(), name: "Read".into(), input: json!({"file_path": "a"}) },
            ])),
            Entry::Message(Message {
                role: Role::User,
                blocks: vec![Block::ToolResult { id: "r".into(), content: "alpha".into(), is_error: false }],
            }),
            Entry::Prompted { at_ms: 9, prompt: "again".into() },
            Entry::Message(Message::user("again")),
        ];
        let replayed = records(entries);
        assert_eq!(replayed[0], Record::Partial);
        assert_eq!(replayed[1], Record::Prompted { at_ms: 0, prompt: "fix the build".into() });
        assert!(replayed.iter().any(
            |r| matches!(r, Record::Event { event: EngineEvent::ToolResult { output, .. }, .. } if output == "alpha")
        ));
        assert_eq!(
            replayed.last(),
            Some(&Record::Prompted { at_ms: 9, prompt: "again".into() }),
            "traced messages are not replayed"
        );
    }

    #[test]
    fn a_traced_session_is_not_marked_partial() {
        let entries = vec![Entry::Prompted { at_ms: 1, prompt: "go".into() }, Entry::Message(Message::user("go"))];
        assert_eq!(records(entries), vec![Record::Prompted { at_ms: 1, prompt: "go".into() }]);
    }
}
