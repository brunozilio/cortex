//! The conversation as the chat shows it: what was said, the model's
//! reasoning, the tools the agent ran, its sub-agents and their steps, and
//! notices. [`Transcript::apply`] folds a run's events into it and keeps
//! what the agent is doing right now ([`Transcript::activity`]).

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
    Thinking { text: String, done: bool },
    Assistant(String),
    Tool { id: String, name: String, summary: String, state: ToolState, output: Option<String> },
    Agent { index: usize, kind: String, summary: String, done: Option<bool>, steps: Vec<String> },
    Notice(String),
    Error(String),
}

#[derive(Debug, Default)]
pub struct Transcript {
    pub entries: Vec<Entry>,
    /// Cost of the runs so far, in US dollars, when the provider prices it.
    pub cost: f64,
    /// Tokens the last model call read: how full the context is.
    pub context: u64,
    /// Tokens the model wrote during the running run.
    pub written: u64,
    /// What the agent is doing now ("Thinking", "Bash(cargo test)").
    pub activity: Option<String>,
    /// Commands the agent left running in the background, by id.
    pub background: Vec<String>,
}

/// How a tool call reads: `Read(src/main.rs)`.
pub fn call(name: &str, summary: &str) -> String {
    if summary.is_empty() { name.to_owned() } else { format!("{name}({summary})") }
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
            EngineEvent::ThinkingDelta(delta) => {
                match self.entries.last_mut() {
                    Some(Entry::Thinking { text, done: false }) => text.push_str(delta),
                    _ => self.push(Entry::Thinking { text: delta.clone(), done: false }),
                }
                self.activity = Some("Thinking".into());
            }
            EngineEvent::TextDelta(delta) => {
                self.close_thinking();
                match self.entries.last_mut() {
                    Some(Entry::Assistant(text)) => text.push_str(delta),
                    _ => self.push(Entry::Assistant(delta.clone())),
                }
                self.activity = Some("Writing".into());
            }
            EngineEvent::ToolStarted(started) => self.started(started),
            EngineEvent::ToolResult { id, output, failed } => {
                self.track_background(output);
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
                self.activity = Some("Working".into());
            }
            EngineEvent::AgentSpawned { index, kind, summary, .. } => {
                self.close_thinking();
                let (index, kind, summary) = (*index, kind.clone(), summary.clone());
                self.activity = Some(format!("{kind}: {summary}"));
                self.push(Entry::Agent { index, kind, summary, done: None, steps: Vec::new() });
            }
            EngineEvent::AgentEvent { index, event } => self.agent_step(*index, event),
            EngineEvent::AgentUpdate { index, status, summary } => self.agent(*index, status, summary.as_deref()),
            EngineEvent::Usage(usage) => {
                self.context = usage.input + usage.cache_read + usage.cache_write;
                self.written += usage.output;
            }
            EngineEvent::Compacted(compaction) => {
                self.notice(format!("Compacted the conversation ({} tokens freed)", compaction.freed()));
            }
            EngineEvent::Diagnosed(problem) => self.error(problem.message()),
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

    /// A new run: nothing written yet.
    pub fn begin(&mut self) {
        self.written = 0;
        self.activity = Some("Working".into());
    }

    fn started(&mut self, started: &ToolCall) {
        self.close_thinking();
        self.activity = Some(call(&started.name, &started.summary));
        self.push(Entry::Tool {
            id: started.id.clone(),
            name: started.name.clone(),
            summary: started.summary.clone(),
            state: ToolState::Running,
            output: None,
        });
    }

    /// Notices the background commands a result started, stopped or saw end.
    fn track_background(&mut self, output: &str) {
        let first = |text: &str| text.split(|c: char| !(c.is_alphanumeric() || c == '_')).next().map(str::to_owned);
        if let Some(id) = output.strip_prefix("Started in the background as ").and_then(first) {
            self.background.push(id);
        } else if let Some(id) = output.strip_prefix("Stopped ").and_then(first) {
            self.background.retain(|known| *known != id);
        } else if let Some((id, state)) = output.strip_prefix('[').and_then(|rest| rest.split_once(": "))
            && state.starts_with("finished")
        {
            self.background.retain(|known| known != id);
        }
    }

    fn close_thinking(&mut self) {
        for entry in self.entries.iter_mut().rev() {
            if let Entry::Thinking { done, .. } = entry {
                *done = true;
                return;
            }
        }
    }

    fn tool(&mut self, id: &str) -> Option<&mut Entry> {
        self.entries.iter_mut().rev().find(|entry| matches!(entry, Entry::Tool { id: known, .. } if known == id))
    }

    fn agent_entry(&mut self, index: usize) -> Option<&mut Entry> {
        self.entries
            .iter_mut()
            .rev()
            .find(|entry| matches!(entry, Entry::Agent { index: known, .. } if *known == index))
    }

    fn agent_step(&mut self, index: usize, event: &EngineEvent) {
        let Some(Entry::Agent { kind, steps, .. }) = self.agent_entry(index) else { return };
        let doing = match event {
            EngineEvent::ToolStarted(started) => {
                let step = call(&started.name, &started.summary);
                steps.push(step.clone());
                step
            }
            EngineEvent::ThinkingDelta(_) => "thinking".to_owned(),
            EngineEvent::TextDelta(_) => "writing".to_owned(),
            _ => return,
        };
        let activity = format!("{kind}: {doing}");
        self.activity = Some(activity);
    }

    fn agent(&mut self, index: usize, status: &AgentStatus, summary: Option<&str>) {
        if let Some(Entry::Agent { summary: kept, done, .. }) = self.agent_entry(index) {
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

    /// The run ended: tools still running get its outcome, reasoning closes.
    fn settle(&mut self, outcome: ToolState) {
        self.close_thinking();
        self.activity = None;
        for entry in &mut self.entries {
            match entry {
                Entry::Tool { state, .. } if *state == ToolState::Running => *state = outcome.clone(),
                Entry::Agent { done: done @ None, .. } => *done = Some(outcome == ToolState::Done),
                _ => {}
            }
        }
    }
}

#[cfg(test)]
#[path = "transcript_tests.rs"]
mod tests;
