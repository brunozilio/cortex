mod agent;
mod drafted;
mod fold;
mod lesson;
mod own_shell;
mod plan;
mod replay;
mod status;
pub mod verdict;

pub use agent::AgentRun;
pub use drafted::drafted;
pub use own_shell::{shell_note, with_shell};
pub use plan::Outcome;
pub use status::{Pending, SessionStatus};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolState {
    Running,
    Done,
    Failed,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum Entry {
    User(String),
    Attached { names: Vec<String> },
    Thinking { text: String, done: bool },
    Assistant(String),
    Tool { id: String, name: String, summary: String, state: ToolState, output: Option<String> },
    Agent { index: usize, kind: String, summary: String, done: Option<bool>, steps: Vec<String> },
    Diff { diff: String, untracked: Vec<String> },
    Plan { plan: String, outcome: Outcome },
    Refused { tool: String, reason: String },
    Repeated { tool: String },
    Finished { duration_ms: u64, cost_usd: Option<f64> },
    Interrupted,
    Compacted { freed: u64 },
    Scheduled { prompt: String },
    Rewound { restored: usize, conversation: bool },
    Branched,
    HeadsUp { urgent: bool, title: String, explain: String },
    Recap(String),
    Lesson { proposal: crate::learning::Proposal, kept: Option<bool> },
    Held { source: String, text: String },
    Notice(String),
    Passing(String),
    Error(String),
}

#[derive(Debug, Default)]
pub struct Transcript {
    pub entries: Vec<Entry>,
    pub cost: f64,
    pub usage: crate::billing::SessionUsage,
    pub context: u64,
    pub last_call: Option<crate::cache_window::LastCall>,
    pub written: u64,
    pub activity: Option<String>,
    pub background: Vec<String>,
    plan_call: Option<String>,
    drafting: Option<String>,
    pub tasks: Vec<crate::events::TaskItem>,
    status: SessionStatus,
    ask: Option<Pending>,
    asking: std::collections::HashMap<crate::AskId, usize>,
    failure: Option<crate::FailureKind>,
    agents: std::collections::BTreeMap<usize, AgentRun>,
    moved: Option<std::path::PathBuf>,
}

fn user_text(entry: &Entry) -> Option<String> {
    match entry {
        Entry::User(text) => Some(text.clone()),
        _ => None,
    }
}

pub const DONE: &str = "✓ Done in ";

pub fn done(duration_ms: u64, cost: Option<f64>) -> String {
    let seconds = duration_ms.div_ceil(1000);
    let took = if seconds < 60 { format!("{seconds}s") } else { format!("{}m {:02}s", seconds / 60, seconds % 60) };
    match cost.filter(|cost| *cost > 0.0) {
        Some(cost) => format!("{DONE}{took} · ${cost:.2}"),
        None => format!("{DONE}{took}"),
    }
}

pub const INTERRUPTED: &str = "Interrupted";

pub fn compacted(freed: u64) -> String {
    format!("Compacted the conversation ({freed} tokens freed)")
}

pub fn call(name: &str, summary: &str) -> String {
    if summary.is_empty() { name.to_owned() } else { format!("{name}({summary})") }
}

impl Transcript {
    pub fn push(&mut self, entry: Entry) {
        self.entries.push(entry);
    }

    pub fn first_prompt(&self) -> Option<String> {
        self.entries.iter().find_map(user_text)
    }

    pub fn last_prompt(&self) -> Option<String> {
        self.entries.iter().rev().find_map(user_text)
    }

    pub fn notice(&mut self, text: impl Into<String>) {
        self.push(Entry::Notice(text.into()));
    }

    pub fn passing(&mut self, text: impl Into<String>) {
        self.push(Entry::Passing(text.into()));
    }

    pub fn replace_passing(&mut self, text: impl Into<String>) {
        self.clear_passing();
        self.passing(text);
    }

    pub fn clear_passing(&mut self) {
        self.entries.retain(|entry| !matches!(entry, Entry::Passing(_)));
    }

    pub fn error(&mut self, text: impl Into<String>) {
        self.push(Entry::Error(text.into()));
    }

    pub fn moved_to(&self) -> Option<&std::path::Path> {
        self.moved.as_deref()
    }

    pub fn begin(&mut self) {
        self.written = 0;
        self.activity = Some("Working".into());
        self.started_run();
    }
}

#[cfg(test)]
mod agent_tests;
#[cfg(test)]
mod held_tests;
#[cfg(test)]
mod plan_tests;
#[cfg(test)]
mod replay_tests;
#[cfg(test)]
mod status_tests;
#[cfg(test)]
mod tests;
