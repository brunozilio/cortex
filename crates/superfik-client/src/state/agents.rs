//! The run's sub-agent cards: spawned by the engine's events, each with its
//! own session transcript, settled when the run ends; and the sheet that
//! shows one agent's session.

use superfik_core::{AgentStatus, CustomAgent, EngineEvent, SubAgent};

use super::{Conversation, Effect, Session};

const UNKNOWN_KIND: &str = "general-purpose";

impl Conversation {
    pub(super) fn apply_agent_event(&mut self, event: &EngineEvent, customs: &[CustomAgent], now: f64) {
        match event {
            EngineEvent::AgentSpawned { index, kind, summary, prompt } => {
                self.spawn_agent(SubAgent::spawned(*index, kind, summary, prompt, customs));
                self.agent_started.insert(*index, now);
            }
            EngineEvent::AgentEvent { index, event } => {
                if let Some(transcript) = self.agent_at(*index).transcript.as_mut() {
                    transcript.response.apply(event);
                }
            }
            EngineEvent::AgentUpdate { index, status, summary } => {
                let agent = self.agent_at(*index);
                if let Some(summary) = summary {
                    agent.summary = summary.clone();
                }
                agent.set_status(status.clone());
            }
            _ => {}
        }
    }

    pub fn opened_agent(&self) -> Option<&SubAgent> {
        self.open_agent.and_then(|index| self.agents.get(index))
    }

    pub fn agent_live_since(&self, index: usize) -> Option<f64> {
        let running = self.agents.get(index).is_some_and(|a| matches!(a.status, AgentStatus::Running { .. }));
        self.agent_started.get(&index).copied().filter(|_| running && self.is_running())
    }

    fn agent_at(&mut self, index: usize) -> &mut SubAgent {
        if self.agents.get(index).is_none() {
            self.spawn_agent(SubAgent::spawned(index, UNKNOWN_KIND, "", "", &[]));
        }
        &mut self.agents[index]
    }

    fn spawn_agent(&mut self, agent: SubAgent) {
        let index = agent.index;
        while self.agents.len() < index {
            let filler = SubAgent::spawned(self.agents.len(), UNKNOWN_KIND, "", "", &[]);
            self.agents.push(filler);
        }
        match self.agents.get_mut(index) {
            Some(slot) => *slot = agent,
            None => self.agents.push(agent),
        }
    }

    pub(super) fn finish_agents(&mut self, elapsed: f64) {
        self.agent_started.clear();
        for agent in &mut self.agents {
            let status = match agent.status {
                AgentStatus::Running { .. } => AgentStatus::Done { seconds: elapsed.max(1.0) as u32 },
                AgentStatus::Queued => AgentStatus::Ready,
                ref other => other.clone(),
            };
            agent.set_status(status);
            if agent.status == AgentStatus::Ready {
                agent.summary = agent.idle_summary();
            }
        }
    }
}

impl Session {
    pub(super) fn toggle_agent(&mut self, index: usize) -> Vec<Effect> {
        let c = self.conversation_mut();
        if index < c.agents.len() {
            c.open_agent = if c.open_agent == Some(index) { None } else { Some(index) };
            c.agent_scroll = 0.0;
            self.popover = None;
        }
        Vec::new()
    }

    pub(super) fn close_agent(&mut self) -> Vec<Effect> {
        self.conversation_mut().open_agent = None;
        Vec::new()
    }

    pub(super) fn scroll_agent(&mut self, by: f32, max: f32) -> Vec<Effect> {
        let c = self.conversation_mut();
        c.agent_scroll = (c.agent_scroll + by).clamp(0.0, max.max(0.0));
        Vec::new()
    }
}
