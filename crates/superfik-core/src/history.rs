//! A session as it is kept between launches: what the timeline and the
//! canvas need to show it again and to continue it with the engine.
//! Whatever only lives while the app runs (the run in flight, the
//! composer, animation state) is left out.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::agents::{AgentStatus, SubAgent};
use crate::context::ResearchItem;
use crate::conversation::{Activity, Turn};
use crate::ids::{ConversationId, SessionId};
use crate::model::Effort;
use crate::plugins::Connection;
use crate::repository::{PullRequest, Repository};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionRecord {
    pub id: ConversationId,
    #[serde(default)]
    pub title: Option<String>,
    pub started_unix: i64,
    #[serde(default)]
    pub turns: Vec<Turn>,
    #[serde(default)]
    pub engine_session: Option<SessionId>,
    #[serde(default)]
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_account: Option<crate::ConnectionId>,
    #[serde(default)]
    pub effort: Effort,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub repositories: Vec<Repository>,
    #[serde(default)]
    pub pull_request: Option<PullRequest>,
    #[serde(default)]
    pub references: Vec<PathBuf>,
    #[serde(default)]
    pub agents: Vec<SubAgent>,
    #[serde(default)]
    pub research_items: Vec<ResearchItem>,
    #[serde(default)]
    pub connections: Vec<Connection>,
    #[serde(default)]
    pub notes: Vec<crate::note::NoteId>,
    #[serde(default)]
    pub visibility: crate::sharing::Visibility,
}

impl SessionRecord {
    pub fn settled(mut self) -> Self {
        for turn in &mut self.turns {
            if turn.response.activity.is_live() {
                turn.response.activity = Activity::Cancelled;
            }
        }
        for agent in &mut self.agents {
            if matches!(agent.status, AgentStatus::Running { .. } | AgentStatus::Queued) {
                agent.status = AgentStatus::Ready;
                agent.summary = agent.idle_summary();
            }
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conversation::UserMessage;

    fn record() -> SessionRecord {
        let mut turn =
            Turn::new(UserMessage { text: "fix it".into(), images: Vec::new(), branch_caption: String::new() });
        turn.response.text = "On it".into();
        let mut agent = SubAgent::new(0);
        agent.status = AgentStatus::Running { progress: 0.4 };
        agent.summary = "Reading".into();
        SessionRecord {
            id: ConversationId::new(),
            title: Some("Fix it".into()),
            started_unix: 1_700_000_000,
            turns: vec![turn],
            engine_session: Some(SessionId("s-1".into())),
            model: "claude-opus-5-5".into(),
            model_account: None,
            effort: Effort::High,
            archived: false,
            repositories: Vec::new(),
            pull_request: None,
            references: vec!["/r/shot.png".into()],
            agents: vec![agent],
            research_items: Vec::new(),
            connections: Vec::new(),
            notes: Vec::new(),
            visibility: Default::default(),
        }
    }

    #[test]
    fn round_trips_through_json() {
        let record = record();
        let json = serde_json::to_string(&record).expect("serialises");
        assert_eq!(serde_json::from_str::<SessionRecord>(&json).expect("parses"), record);
    }

    #[test]
    fn missing_fields_take_defaults() {
        let id = ConversationId::new();
        let json = format!(r#"{{"id":{},"started_unix":5}}"#, serde_json::to_string(&id).expect("id"));
        let record: SessionRecord = serde_json::from_str(&json).expect("parses");
        assert_eq!(record.id, id);
        assert!(record.turns.is_empty() && !record.archived);
    }

    #[test]
    fn settling_cancels_live_replies_and_idles_agents() {
        let settled = record().settled();
        assert_eq!(settled.turns[0].response.activity, Activity::Cancelled);
        assert_eq!(settled.agents[0].status, AgentStatus::Ready);
        assert_eq!(settled.agents[0].summary, settled.agents[0].idle_summary());

        let mut done = record();
        done.turns[0].response.activity = Activity::Done;
        assert_eq!(done.settled().turns[0].response.activity, Activity::Done);
    }
}
