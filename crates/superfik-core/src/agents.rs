//! Sub-agents spawned by a run (the nodes on the right of the canvas).

use serde::{Deserialize, Serialize};

use crate::conversation::{Activity, Turn, UserMessage};
use crate::ids::AgentId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentRole {
    Explorer,
    Builder,
    Reviewer,
    Tester,
}

impl AgentRole {
    pub const ROSTER: [AgentRole; 4] = [Self::Explorer, Self::Builder, Self::Reviewer, Self::Tester];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Explorer => "Explorer",
            Self::Builder => "Builder",
            Self::Reviewer => "Reviewer",
            Self::Tester => "Tester",
        }
    }

    pub const fn idle_summary(self) -> &'static str {
        match self {
            Self::Explorer => "Mapping the codebase",
            Self::Builder => "Drafting the change",
            Self::Reviewer => "Checking the diff",
            Self::Tester => "Running the checks",
        }
    }

    pub fn for_index(index: usize) -> Self {
        Self::ROSTER[index % Self::ROSTER.len()]
    }
}

const EXPLORE_KIND: &str = "Explore";

pub const WAITING_TURN: &str = "Waiting its turn";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AgentStatus {
    Ready,
    Queued,
    Running { progress: f32 },
    Done { seconds: u32 },
    Failed,
}

impl AgentStatus {
    pub fn label(&self) -> Option<&'static str> {
        match self {
            Self::Ready => Some("Ready"),
            Self::Queued => Some("Queued"),
            Self::Failed => Some("Failed"),
            Self::Running { .. } | Self::Done { .. } => None,
        }
    }

    pub fn progress(&self) -> f32 {
        match self {
            Self::Running { progress } => progress.clamp(0.0, 1.0),
            Self::Done { .. } => 1.0,
            Self::Ready | Self::Queued | Self::Failed => 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomAgent {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubAgent {
    pub id: AgentId,
    pub index: usize,
    pub role: AgentRole,
    pub status: AgentStatus,
    pub summary: String,
    #[serde(default)]
    pub custom: Option<CustomAgent>,
    #[serde(default)]
    pub transcript: Option<Turn>,
}

impl SubAgent {
    pub fn new(index: usize) -> Self {
        let role = AgentRole::for_index(index);
        Self {
            id: AgentId::new(),
            index,
            role,
            status: AgentStatus::Ready,
            summary: role.idle_summary().to_owned(),
            custom: None,
            transcript: None,
        }
    }

    pub fn for_slot(index: usize, customs: &[CustomAgent]) -> Self {
        let mut agent = Self::new(index);
        match customs.get(index) {
            Some(custom) => agent.custom = Some(custom.clone()),
            None => agent.role = AgentRole::for_index(index - customs.len().min(index)),
        }
        agent.summary = agent.idle_summary();
        agent
    }

    pub fn spawned(index: usize, kind: &str, task: &str, prompt: &str, customs: &[CustomAgent]) -> Self {
        let mut agent = Self::new(index);
        agent.role = AgentRole::Explorer;
        if kind != EXPLORE_KIND {
            let custom = customs.iter().find(|custom| custom.name == kind).cloned();
            agent.custom =
                Some(custom.unwrap_or_else(|| CustomAgent { name: kind.to_owned(), description: String::new() }));
        }
        agent.status = AgentStatus::Running { progress: 0.0 };
        agent.summary = if task.is_empty() { agent.idle_summary() } else { task.to_owned() };
        let text = if prompt.trim().is_empty() { task } else { prompt.trim() };
        agent.transcript =
            Some(Turn::new(UserMessage { text: text.to_owned(), images: Vec::new(), branch_caption: String::new() }));
        agent
    }

    pub fn set_status(&mut self, status: AgentStatus) {
        if let Some(response) = self.transcript.as_mut().map(|turn| &mut turn.response) {
            match status {
                AgentStatus::Done { seconds } => {
                    response.activity = Activity::Done;
                    response.elapsed_ms = u64::from(seconds) * 1000;
                }
                AgentStatus::Failed => response.activity = Activity::Failed(self.summary.clone()),
                AgentStatus::Ready | AgentStatus::Queued | AgentStatus::Running { .. } => {}
            }
        }
        self.status = status;
    }

    pub fn idle_summary(&self) -> String {
        match &self.custom {
            Some(custom) if !custom.description.is_empty() => custom.description.clone(),
            _ => self.role.idle_summary().to_owned(),
        }
    }

    pub fn label(&self) -> &str {
        self.custom.as_ref().map_or(self.role.label(), |custom| custom.name.as_str())
    }

    pub fn title(&self) -> String {
        format!("Agent {}", self.index + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roster_cycles() {
        assert_eq!(AgentRole::for_index(0), AgentRole::Explorer);
        assert_eq!(AgentRole::for_index(2), AgentRole::Reviewer);
        assert_eq!(AgentRole::for_index(4), AgentRole::Explorer);
    }

    #[test]
    fn progress_is_clamped_and_complete_when_done() {
        assert_eq!(AgentStatus::Running { progress: 1.4 }.progress(), 1.0);
        assert_eq!(AgentStatus::Done { seconds: 5 }.progress(), 1.0);
        assert_eq!(AgentStatus::Queued.progress(), 0.0);
    }

    #[test]
    fn new_agent_is_ready_with_role_summary() {
        let agent = SubAgent::new(1);
        assert_eq!(agent.title(), "Agent 2");
        assert_eq!(agent.role, AgentRole::Builder);
        assert_eq!(agent.status.label(), Some("Ready"));
    }

    #[test]
    fn custom_agents_fill_the_first_slots() {
        let customs = vec![CustomAgent { name: "code-reviewer".into(), description: "Reviews diffs".into() }];
        let first = SubAgent::for_slot(0, &customs);
        assert_eq!(first.label(), "code-reviewer");
        assert_eq!(first.summary, "Reviews diffs");
        let second = SubAgent::for_slot(1, &customs);
        assert_eq!(second.label(), "Explorer");
        assert_eq!(SubAgent::for_slot(2, &[]).role, AgentRole::Reviewer);
    }

    #[test]
    fn spawned_agents_show_their_kind_and_task() {
        let customs = vec![CustomAgent { name: "code-reviewer".into(), description: "Reviews diffs".into() }];
        let explorer = SubAgent::spawned(0, "Explore", "Map the parser", "Map how the parser works", &customs);
        assert_eq!((explorer.label(), explorer.custom.is_none()), ("Explorer", true));
        assert_eq!(explorer.summary, "Map the parser");
        assert_eq!(explorer.status, AgentStatus::Running { progress: 0.0 });
        assert_eq!(explorer.transcript.map(|t| t.user.text).as_deref(), Some("Map how the parser works"));
        let reviewer = SubAgent::spawned(1, "code-reviewer", "", "", &customs);
        assert_eq!((reviewer.label(), reviewer.summary.as_str()), ("code-reviewer", "Reviews diffs"));
        assert_eq!(SubAgent::spawned(2, "general-purpose", "Fix it", "", &customs).label(), "general-purpose");
    }

    #[test]
    fn finishing_settles_the_transcript() {
        let mut agent = SubAgent::spawned(0, "Explore", "Map", "Map it", &[]);
        agent.set_status(AgentStatus::Running { progress: 0.4 });
        assert!(agent.transcript.as_ref().is_some_and(|t| t.response.activity.is_live()));
        agent.set_status(AgentStatus::Done { seconds: 3 });
        let response = agent.transcript.map(|t| t.response);
        assert_eq!(response.map(|r| (r.activity, r.elapsed_ms)), Some((Activity::Done, 3000)));
        let mut failed = SubAgent::spawned(1, "Explore", "boom", "", &[]);
        failed.set_status(AgentStatus::Failed);
        assert_eq!(failed.transcript.map(|t| t.response.activity), Some(Activity::Failed("boom".into())));
    }
}
