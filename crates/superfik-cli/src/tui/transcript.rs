//! The conversation as the chat shows it: what was said, the tools the
//! agent ran, its sub-agents, and notices. [`Transcript::apply`] folds a
//! run's events into it.

use superfik_core::{AgentStatus, ToolCall};
use superfik_sdk::EngineEvent;

#[derive(Debug, Clone, PartialEq)]
pub enum ToolState {
    Running,
    Done,
    Failed,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Entry {
    User(String),
    Assistant(String),
    Tool { id: String, name: String, summary: String, state: ToolState, output: Option<String> },
    Agent { index: usize, kind: String, summary: String, done: Option<bool> },
    Notice(String),
    Error(String),
}

#[derive(Debug, Default)]
pub struct Transcript {
    pub entries: Vec<Entry>,
    /// Cost of the runs so far, in US dollars, when the provider prices it.
    pub cost: f64,
}

impl Transcript {
    pub fn push(&mut self, entry: Entry) {
        self.entries.push(entry);
    }

    pub fn notice(&mut self, text: impl Into<String>) {
        self.push(Entry::Notice(text.into()));
    }

    pub fn error(&mut self, text: impl Into<String>) {
        self.push(Entry::Error(text.into()));
    }

    /// Folds one event of the running run in. Asks are the chat's to
    /// handle; they leave the transcript as it is.
    pub fn apply(&mut self, event: &EngineEvent) {
        match event {
            EngineEvent::TextDelta(delta) => match self.entries.last_mut() {
                Some(Entry::Assistant(text)) => text.push_str(delta),
                _ => self.push(Entry::Assistant(delta.clone())),
            },
            EngineEvent::ToolStarted(call) => self.started(call),
            EngineEvent::ToolResult { id, output, failed } => {
                if let Some(Entry::Tool { state, output: kept, .. }) = self.tool(id) {
                    *kept = Some(output.clone());
                    if *failed {
                        *state = ToolState::Failed;
                    }
                }
            }
            EngineEvent::ToolFinished { id } => {
                if let Some(Entry::Tool { state, .. }) = self.tool(id)
                    && *state == ToolState::Running
                {
                    *state = ToolState::Done;
                }
            }
            EngineEvent::AgentSpawned { index, kind, summary, .. } => {
                self.push(Entry::Agent { index: *index, kind: kind.clone(), summary: summary.clone(), done: None });
            }
            EngineEvent::AgentUpdate { index, status, summary } => self.agent(*index, status, summary.as_deref()),
            EngineEvent::Compacted(compaction) => {
                self.notice(format!("Compacted the conversation ({} tokens freed)", compaction.freed()));
            }
            EngineEvent::Diagnosed(problem) => {
                self.error(problem.message(superfik_core::EngineKind::Native));
            }
            EngineEvent::Completed { cost_usd, .. } => {
                self.cost += cost_usd.unwrap_or(0.0);
                self.settle(ToolState::Done);
            }
            EngineEvent::Failed(message) => {
                self.settle(ToolState::Failed);
                self.error(message.clone());
            }
            EngineEvent::Cancelled => {
                self.settle(ToolState::Failed);
                self.notice("Interrupted");
            }
            _ => {}
        }
    }

    fn started(&mut self, call: &ToolCall) {
        self.push(Entry::Tool {
            id: call.id.clone(),
            name: call.name.clone(),
            summary: call.summary.clone(),
            state: ToolState::Running,
            output: None,
        });
    }

    fn tool(&mut self, id: &str) -> Option<&mut Entry> {
        self.entries.iter_mut().rev().find(|entry| matches!(entry, Entry::Tool { id: known, .. } if known == id))
    }

    fn agent(&mut self, index: usize, status: &AgentStatus, summary: Option<&str>) {
        let found = self
            .entries
            .iter_mut()
            .rev()
            .find(|entry| matches!(entry, Entry::Agent { index: known, .. } if *known == index));
        if let Some(Entry::Agent { summary: kept, done, .. }) = found {
            if let Some(summary) = summary {
                *kept = summary.to_owned();
            }
            match status {
                AgentStatus::Done { .. } => *done = Some(true),
                AgentStatus::Failed => *done = Some(false),
                _ => {}
            }
        }
    }

    /// Tools still running when a run ends get its outcome.
    fn settle(&mut self, outcome: ToolState) {
        for entry in &mut self.entries {
            if let Entry::Tool { state, .. } = entry
                && *state == ToolState::Running
            {
                *state = outcome.clone();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_streams_into_one_reply_and_tools_settle() {
        let mut transcript = Transcript::default();
        transcript.push(Entry::User("hi".into()));
        for event in [
            EngineEvent::TextDelta("Let me ".into()),
            EngineEvent::TextDelta("look.".into()),
            EngineEvent::ToolStarted(ToolCall::new("t1", "Read", "src/main.rs")),
            EngineEvent::ToolResult { id: "t1".into(), output: "fn main".into(), failed: false },
            EngineEvent::ToolFinished { id: "t1".into() },
            EngineEvent::ToolStarted(ToolCall::new("t2", "Bash", "cargo test")),
            EngineEvent::TextDelta("Done.".into()),
            EngineEvent::Completed { duration_ms: 1, cost_usd: Some(0.25) },
        ] {
            transcript.apply(&event);
        }
        assert_eq!(transcript.entries[1], Entry::Assistant("Let me look.".into()));
        assert!(
            matches!(&transcript.entries[2], Entry::Tool { state: ToolState::Done, output: Some(o), .. } if o == "fn main")
        );
        assert!(matches!(&transcript.entries[3], Entry::Tool { state: ToolState::Done, .. }), "settled at the end");
        assert_eq!(transcript.entries[4], Entry::Assistant("Done.".into()), "text after a tool is a new reply");
        assert!((transcript.cost - 0.25).abs() < f64::EPSILON);
    }

    #[test]
    fn a_failed_tool_a_sub_agent_and_an_interrupt_show() {
        let mut transcript = Transcript::default();
        for event in [
            EngineEvent::ToolStarted(ToolCall::new("t1", "Bash", "false")),
            EngineEvent::ToolResult { id: "t1".into(), output: "exit 1".into(), failed: true },
            EngineEvent::ToolFinished { id: "t1".into() },
            EngineEvent::AgentSpawned {
                index: 0,
                kind: "Explore".into(),
                summary: "Find tests".into(),
                prompt: String::new(),
            },
            EngineEvent::AgentUpdate {
                index: 0,
                status: AgentStatus::Done { seconds: 3 },
                summary: Some("Found 4".into()),
            },
            EngineEvent::Cancelled,
        ] {
            transcript.apply(&event);
        }
        assert!(matches!(&transcript.entries[0], Entry::Tool { state: ToolState::Failed, .. }));
        assert_eq!(
            transcript.entries[1],
            Entry::Agent { index: 0, kind: "Explore".into(), summary: "Found 4".into(), done: Some(true) }
        );
        assert_eq!(transcript.entries[2], Entry::Notice("Interrupted".into()));
    }
}
