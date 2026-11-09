//! One conversation with an agent inside the workspace: its transcript,
//! the run in flight, the composer and the context attached to the next
//! prompt. The UI calls it a session.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;

use superfik_core::{
    AgentStatus, Compaction, Connection, ContextUsage, ConversationId, CustomAgent, Effort, EngineEvent,
    PermissionMode, PullRequest, Repository, ResearchItem, RunId, SessionId, Skill, SubAgent, Turn, TurnId,
};
use superfik_ui::TextEdit;

use super::activity::{Activity, Context, ContextNode};
use super::ask::{self, PendingAsk};
use super::context::skills_in;
use super::upkeep::Upkeep;

#[derive(Debug, Clone, PartialEq)]
pub struct ActiveRun {
    pub id: RunId,
    pub started_at: f64,
}

#[derive(Debug, Clone)]
pub struct Conversation {
    pub id: ConversationId,
    pub title: Option<String>,
    pub started_unix: Option<i64>,
    pub turns: Vec<Turn>,
    pub run: Option<ActiveRun>,
    pub finished_at: Option<f64>,
    pub engine_session: Option<SessionId>,
    pub context: Option<ContextUsage>,
    pub compaction: Option<Compaction>,
    pub upkeep: Option<Upkeep>,
    pub chores: super::chores::Chores,
    pub agents: Vec<SubAgent>,
    pub open_agent: Option<usize>,
    pub agent_scroll: f32,
    pub agent_started: HashMap<usize, f64>,
    pub research_items: Vec<ResearchItem>,
    pub activity: Activity,
    pub open_steps: HashMap<TurnId, f64>,
    pub open_outputs: std::collections::HashSet<(TurnId, usize)>,
    pub composer: TextEdit,
    pub ended_unseen: bool,
    pub removed_skills: Vec<String>,
    pub pages: HashMap<super::Carousel, usize>,
    pub repositories: Vec<Repository>,
    pub pull_request: Option<PullRequest>,
    pub references: Vec<PathBuf>,
    pub uploading: Vec<PathBuf>,
    pub previews: HashMap<PathBuf, PathBuf>,
    pub expanded_reference: Option<usize>,
    pub model: usize,
    pub wanted_model: Option<(String, Option<superfik_core::ConnectionId>)>,
    pub effort: Effort,
    pub archived: bool,
    pub asks: VecDeque<PendingAsk>,
    pub permission: PermissionMode,
    pub connections: Vec<Connection>,
    pub notes: Vec<superfik_core::note::NoteId>,
    pub visibility: superfik_core::sharing::Visibility,
    pub shared_by: Option<String>,
    pub carried: Option<String>,
    pub browser: Option<super::browser::BrowserCard>,
}

impl Conversation {
    pub fn new(model: usize, effort: Effort) -> Self {
        Self {
            id: ConversationId::new(),
            title: None,
            started_unix: None,
            turns: Vec::new(),
            run: None,
            finished_at: None,
            engine_session: None,
            context: None,
            compaction: None,
            upkeep: None,
            chores: Default::default(),
            agents: Vec::new(),
            open_agent: None,
            agent_scroll: 0.0,
            agent_started: HashMap::new(),
            research_items: Vec::new(),
            activity: Activity::default(),
            open_steps: HashMap::new(),
            open_outputs: std::collections::HashSet::new(),
            composer: TextEdit::default(),
            ended_unseen: false,
            removed_skills: Vec::new(),
            pages: HashMap::new(),
            repositories: Vec::new(),
            pull_request: None,
            references: Vec::new(),
            uploading: Vec::new(),
            previews: HashMap::new(),
            expanded_reference: None,
            model,
            wanted_model: None,
            effort,
            archived: false,
            asks: VecDeque::new(),
            permission: PermissionMode::default(),
            connections: Vec::new(),
            notes: Vec::new(),
            visibility: Default::default(),
            shared_by: None,
            carried: None,
            browser: None,
        }
    }

    pub fn is_disposable(&self) -> bool {
        self.is_empty() && !self.is_running() && self.composer.is_empty()
    }

    pub fn is_running(&self) -> bool {
        self.run.is_some()
    }

    pub fn is_waiting(&self) -> bool {
        !self.asks.is_empty()
    }

    pub fn is_empty(&self) -> bool {
        self.turns.is_empty()
    }

    pub fn owns_run(&self, run: RunId) -> bool {
        self.run.as_ref().is_some_and(|active| active.id == run)
    }

    pub fn is_node_active(&self, node: ContextNode, now: f64) -> bool {
        self.is_running() && self.activity.is_active(node, now)
    }

    pub fn is_gathering(&self) -> bool {
        let started = |a: &SubAgent| matches!(a.status, AgentStatus::Running { .. } | AgentStatus::Done { .. });
        self.is_running()
            && self.turns.last().is_none_or(|t| t.response.text.is_empty())
            && !self.agents.iter().any(started)
    }

    pub fn selected_skills(&self, available: &[Skill]) -> Vec<Skill> {
        let typed = skills_in(self.composer.text(), available);
        available
            .iter()
            .filter(|s| !self.removed_skills.contains(&s.name) || typed.iter().any(|t| t.name == s.name))
            .cloned()
            .collect()
    }

    pub fn preview<'a>(&'a self, reference: &'a std::path::Path) -> &'a std::path::Path {
        self.previews.get(reference).map_or(reference, PathBuf::as_path)
    }

    pub fn shows_research(&self) -> bool {
        !self.research_items.is_empty() || (self.is_running() && self.activity.is_open(ContextNode::Research))
    }

    pub fn apply_run_event(&mut self, event: &EngineEvent, customs: &[CustomAgent], now: f64) -> Option<ActiveRun> {
        let active = self.run.clone()?;
        let context = Context { references: &self.references };
        self.activity.observe(event, context, now);
        match event {
            EngineEvent::SessionStarted(id) => self.engine_session = Some(id.clone()),
            EngineEvent::ResearchFound(item) => {
                if !self.research_items.iter().any(|i| i.title == item.title) {
                    self.research_items.push(item.clone());
                }
            }
            EngineEvent::Asked(asked) => ask::enqueue(&mut self.asks, active.id, asked, now),
            EngineEvent::AskWithdrawn(id) => ask::withdraw(&mut self.asks, id),
            EngineEvent::AgentSpawned { .. } | EngineEvent::AgentEvent { .. } | EngineEvent::AgentUpdate { .. } => {
                self.apply_agent_event(event, customs, now);
            }
            _ => {}
        }
        if let Some(turn) = self.turns.last_mut() {
            turn.response.apply(event);
            if turn.response.elapsed_ms == 0 && event.is_terminal() {
                turn.response.elapsed_ms = ((now - active.started_at) * 1000.0) as u64;
            }
        }
        if event.is_terminal() {
            self.asks.clear();
            self.finish_agents(now - active.started_at);
            self.run = None;
            self.finished_at = Some(now);
        }
        Some(active)
    }
}
