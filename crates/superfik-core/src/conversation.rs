//! A conversation is a list of turns: a user message and the engine's reply.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::events::{EngineEvent, ToolCall};
use crate::failure::FailureKind;
use crate::ids::TurnId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserMessage {
    pub text: String,
    pub images: Vec<PathBuf>,
    pub branch_caption: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Activity {
    Thinking,
    SearchingWeb,
    Reading,
    Writing,
    Done,
    Failed(String),
    Cancelled,
}

impl Activity {
    pub fn is_live(&self) -> bool {
        matches!(self, Self::Thinking | Self::SearchingWeb | Self::Reading | Self::Writing)
    }

    pub fn label(&self, elapsed_ms: u64) -> String {
        match self {
            Self::Thinking => "Thinking".into(),
            Self::SearchingWeb => "Searching the web".into(),
            Self::Reading => "Reading files".into(),
            Self::Writing => "Writing".into(),
            Self::Done => format!("Worked for {}", format_seconds(elapsed_ms)),
            Self::Failed(_) => "Stopped with an error".into(),
            Self::Cancelled => "Stopped".into(),
        }
    }
}

const TITLE_CHARS: usize = 56;

pub fn title_from_prompt(text: &str) -> Option<String> {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or_default();
    let words: Vec<&str> = line.split_whitespace().skip_while(|w| w.starts_with('/') && w.len() > 1).collect();
    let full = words.join(" ");
    if full.chars().count() <= TITLE_CHARS {
        return (!full.is_empty()).then_some(full);
    }
    let mut title = String::new();
    for word in &words {
        let next = if title.is_empty() { (*word).to_owned() } else { format!("{title} {word}") };
        if next.chars().count() > TITLE_CHARS - 1 {
            break;
        }
        title = next;
    }
    if title.is_empty() {
        title = full.chars().take(TITLE_CHARS - 1).collect();
    }
    Some(title + "…")
}

pub fn format_seconds(ms: u64) -> String {
    let secs = (ms / 1000).max(1);
    if secs < 60 { format!("{secs}s") } else { format!("{}m {}s", secs / 60, secs % 60) }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TurnResponse {
    #[serde(default)]
    pub usage: Vec<crate::billing::ModelUsage>,
    #[serde(default)]
    pub structured: Option<serde_json::Value>,
    pub activity: Activity,
    pub text: String,
    pub tools: Vec<ToolCall>,
    pub elapsed_ms: u64,
    #[serde(default)]
    pub failure: Option<FailureKind>,
}

impl Default for TurnResponse {
    fn default() -> Self {
        Self {
            usage: Vec::new(),
            structured: None,
            activity: Activity::Thinking,
            text: String::new(),
            tools: Vec::new(),
            elapsed_ms: 0,
            failure: None,
        }
    }
}

impl TurnResponse {
    pub fn steps(&self) -> usize {
        self.tools.len() + usize::from(!self.text.is_empty())
    }

    pub fn apply(&mut self, event: &EngineEvent) -> bool {
        match event {
            EngineEvent::Usage(usage) => self.usage.push(usage.clone()),
            EngineEvent::StructuredOutput(value) => {
                self.structured = Some(value.clone());
                self.text = serde_json::to_string_pretty(value).unwrap_or_default();
            }
            EngineEvent::TextDelta(delta) => {
                if self.structured.is_none() {
                    self.text.push_str(delta);
                }
                self.activity = Activity::Writing;
            }
            EngineEvent::ToolResult { id, output, failed } => {
                if let Some(call) = self.tools.iter_mut().rev().find(|call| call.id == *id) {
                    call.output = Some(output.clone());
                    call.failed = *failed;
                }
            }
            EngineEvent::ToolStarted(call) => {
                self.activity = if call.is_web() { Activity::SearchingWeb } else { Activity::Reading };
                self.tools.push(call.clone());
            }
            EngineEvent::Completed { duration_ms, .. } => {
                self.activity = Activity::Done;
                self.elapsed_ms = *duration_ms;
            }
            EngineEvent::Failure(kind) => self.failure = Some(*kind),
            EngineEvent::Failed(message) => self.activity = Activity::Failed(message.clone()),
            EngineEvent::Cancelled => self.activity = Activity::Cancelled,
            EngineEvent::SessionStarted(_)
            | EngineEvent::ThinkingDelta(_)
            | EngineEvent::ToolFinished { .. }
            | EngineEvent::ResearchFound(_)
            | EngineEvent::AgentSpawned { .. }
            | EngineEvent::AgentEvent { .. }
            | EngineEvent::AgentUpdate { .. }
            | EngineEvent::Diagnosed(_)
            | EngineEvent::ContextMeasured(_)
            | EngineEvent::Compacted(_)
            | EngineEvent::Chore(_)
            | EngineEvent::Asked(_)
            | EngineEvent::AskWithdrawn(_) => return false,
        }
        true
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Turn {
    pub id: TurnId,
    pub user: UserMessage,
    pub response: TurnResponse,
    #[serde(default)]
    pub author: Option<String>,
}

impl Turn {
    pub fn new(user: UserMessage) -> Self {
        Self { id: TurnId::new(), user, response: TurnResponse::default(), author: None }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SessionId;

    #[test]
    fn titles_come_from_the_first_line_without_skill_tokens() {
        assert_eq!(
            title_from_prompt("/motion /pdf  Create a small night sky.\nMore").as_deref(),
            Some("Create a small night sky.")
        );
        assert_eq!(title_from_prompt("\n\n  hey  ").as_deref(), Some("hey"));
        assert_eq!(title_from_prompt("   "), None);
        assert_eq!(title_from_prompt("/").as_deref(), Some("/"));
    }

    #[test]
    fn long_titles_are_cut_at_a_word_boundary() {
        let long = "Refactor the renderer so that every goo primitive carries its own clip rectangle please";
        let title = title_from_prompt(long).unwrap_or_default();
        assert!(title.ends_with('…'));
        assert!(title.chars().count() <= TITLE_CHARS);
        assert_eq!(title, "Refactor the renderer so that every goo primitive…");
        let unbroken = "x".repeat(80);
        assert_eq!(title_from_prompt(&unbroken).unwrap_or_default().chars().count(), TITLE_CHARS);
    }

    #[test]
    fn activity_labels() {
        assert_eq!(Activity::Done.label(3_400), "Worked for 3s");
        assert_eq!(Activity::Done.label(200), "Worked for 1s");
        assert_eq!(Activity::Done.label(75_000), "Worked for 1m 15s");
        assert_eq!(Activity::SearchingWeb.label(0), "Searching the web");
    }

    #[test]
    fn response_folds_events() {
        let mut response = TurnResponse::default();
        assert!(!response.apply(&EngineEvent::SessionStarted(SessionId("s".into()))));
        response.apply(&EngineEvent::ToolStarted(ToolCall::new("1", "WebSearch", "night skies")));
        assert_eq!(response.activity, Activity::SearchingWeb);
        response.apply(&EngineEvent::TextDelta("Hey".into()));
        response.apply(&EngineEvent::TextDelta("!".into()));
        assert_eq!(response.text, "Hey!");
        assert_eq!(response.activity, Activity::Writing);
        response.apply(&EngineEvent::Completed { duration_ms: 3000, cost_usd: None });
        assert_eq!(response.activity, Activity::Done);
        assert_eq!(response.steps(), 2);
    }
}
