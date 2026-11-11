use super::{Builder, asked_tool, lane, node};
use crate::trace::{Agent, Kind, ModelCall, Node, RunInfo};
use crate::{AgentAsk, AgentStatus, Rewound};

impl Builder<'_> {
    pub(super) fn opened(&mut self, info: RunInfo) {
        let prompt =
            self.steps.iter_mut().rev().flat_map(|step| step.nodes.iter_mut()).find_map(|node| match &mut node.kind {
                Kind::Prompt { run, .. } => Some(run),
                _ => None,
            });
        if let Some(run) = prompt {
            *run = Some(info);
        }
    }

    pub(super) fn called(&mut self, call: ModelCall) {
        self.model = Some(call.model.clone());
        if !self.running() {
            self.asked = Some(call);
            return;
        }
        if let Some(Kind::Tool { calls, .. }) = self.waiting_tool(false).map(|tool| &mut tool.kind) {
            calls.push(call);
        }
    }

    pub(super) fn ask(&mut self, at: u64, ask: AgentAsk) {
        let Some(name) = asked_tool(&ask).map(str::to_owned) else {
            return self.note(at, "asked", format!("{:?}", ask.kind));
        };
        let running =
            self.group.iter_mut().rev().find(|tool| {
                tool.until_ms.is_none() && matches!(&tool.kind, Kind::Tool { call, .. } if call.name == name)
            });
        match running.map(|tool| &mut tool.kind) {
            Some(Kind::Tool { asks, .. }) => asks.push(ask),
            _ => self.asks.push(ask),
        }
    }

    pub(super) fn rewound(&mut self, at: u64, rewound: Rewound) {
        let restored: Vec<String> = rewound.restored.iter().map(|path| path.display().to_string()).collect();
        let last = self.steps.last_mut().and_then(|step| step.nodes.last_mut()).map(|node| &mut node.kind);
        if let Some(Kind::Rewound { checkpoint: checkpoint @ None, prompt, restored: kept, .. }) = last {
            (*checkpoint, *prompt, *kept) = (Some(rewound.checkpoint), Some(rewound.prompt), restored);
            return;
        }
        let kind =
            Kind::Rewound { keep: None, checkpoint: Some(rewound.checkpoint), prompt: Some(rewound.prompt), restored };
        self.single(node(at, false, kind));
    }

    pub(super) fn waiting_tool(&mut self, without_agent: bool) -> Option<&mut Node> {
        let free = |tool: &Node| {
            tool.until_ms.is_none()
                && matches!(&tool.kind, Kind::Tool { agent, .. } if !without_agent || agent.is_none())
        };
        let named = self
            .group
            .iter()
            .position(|tool| free(tool) && matches!(&tool.kind, Kind::Tool { call, .. } if call.name == "Agent"));
        let at = named.or_else(|| self.group.iter().position(free))?;
        self.group.get_mut(at)
    }

    fn returned_agent_call(&mut self) -> Option<&mut Option<Box<Agent>>> {
        let earlier = self.steps.iter_mut().rev().flat_map(|step| step.nodes.iter_mut().rev());
        self.group.iter_mut().rev().chain(earlier).find_map(|node| match &mut node.kind {
            Kind::Tool { call, agent, .. } if call.name == "Agent" && agent.is_none() => Some(agent),
            _ => None,
        })
    }

    pub(super) fn spawned(&mut self, at: u64, index: usize) {
        let spawn = self.agents.remove(&index).unwrap_or_default();
        let failed = spawn.status == Some(AgentStatus::Failed);
        let agent = Box::new(Agent {
            name: spawn.kind,
            summary: spawn.summary,
            prompt: spawn.prompt,
            status: spawn.status,
            lane: lane(spawn.items, self.agents),
        });
        if let Some(Node { kind: Kind::Tool { agent: slot, .. }, .. }) = self.waiting_tool(true) {
            *slot = Some(agent);
        } else if let Some(slot) = self.returned_agent_call() {
            *slot = Some(agent);
        } else {
            self.single(node(at, failed, Kind::Agent(agent)));
        }
    }
}
