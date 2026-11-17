use super::{Entry, ToolState, Transcript};
use crate::{AgentStatus, EngineEvent};

#[derive(Debug)]
pub struct AgentRun {
    pub kind: String,
    pub summary: String,
    pub prompt: String,
    pub status: AgentStatus,
    said: Transcript,
}

impl AgentRun {
    pub fn entries(&self) -> &[Entry] {
        &self.said.entries
    }

    fn running(&self) -> bool {
        matches!(self.status, AgentStatus::Running { .. })
    }

    fn end(&mut self, status: AgentStatus) {
        let outcome = if matches!(status, AgentStatus::Done { .. }) { ToolState::Done } else { ToolState::Failed };
        self.said.settle(outcome);
        self.status = status;
    }
}

impl Transcript {
    pub fn agent(&self, index: usize) -> Option<&AgentRun> {
        self.agents.get(&index)
    }

    pub fn agents(&self) -> impl Iterator<Item = (usize, &AgentRun)> {
        self.agents.iter().map(|(index, agent)| (*index, agent))
    }

    pub fn running_agents(&self) -> Vec<usize> {
        self.agents.iter().filter(|(_, agent)| agent.running()).map(|(index, _)| *index).collect()
    }

    pub(super) fn follow_agents(&mut self, event: &EngineEvent) {
        match event {
            EngineEvent::AgentSpawned { index, kind, summary, prompt } => {
                self.agents.insert(
                    *index,
                    AgentRun {
                        kind: kind.clone(),
                        summary: summary.clone(),
                        prompt: prompt.clone(),
                        status: AgentStatus::Running { progress: 0.0 },
                        said: Transcript::default(),
                    },
                );
            }
            EngineEvent::AgentEvent { index, event } => {
                if let Some(agent) = self.agents.get_mut(index)
                    && !matches!(event.as_ref(), EngineEvent::Asked(_))
                {
                    agent.said.apply(event);
                }
            }
            EngineEvent::AgentUpdate { index, status, summary } => {
                if let Some(agent) = self.agents.get_mut(index) {
                    if let Some(summary) = summary {
                        agent.summary.clone_from(summary);
                    }
                    if agent.running() || matches!(status, AgentStatus::Running { .. }) {
                        match status {
                            AgentStatus::Running { .. } => agent.status = status.clone(),
                            ended => agent.end(ended.clone()),
                        }
                    }
                }
            }
            terminal if terminal.is_terminal() => self.end_agents(&AgentStatus::Cancelled),
            _ => {}
        }
    }

    fn end_agents(&mut self, status: &AgentStatus) {
        for index in self.running_agents() {
            if let Some(agent) = self.agents.get_mut(&index) {
                agent.end(status.clone());
            }
        }
    }
}
