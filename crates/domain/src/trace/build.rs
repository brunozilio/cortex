use std::collections::HashMap;

use super::{Agent, Kind, Lane, Node, Record, Step, Totals, Trace};
use crate::billing::ModelUsage;
use crate::{AgentStatus, EngineEvent};

enum Item {
    Prompt(String),
    Event(EngineEvent),
    Truncated(usize),
    Spawned(usize),
}

#[derive(Default)]
struct Spawn {
    kind: String,
    summary: String,
    prompt: String,
    status: Option<AgentStatus>,
    items: Vec<(u64, Item)>,
}

#[derive(Default)]
struct Routed {
    main: Vec<(u64, Item)>,
    agents: HashMap<usize, Spawn>,
    totals: Totals,
}

impl Routed {
    fn push(&mut self, owner: Option<usize>, at_ms: u64, item: Item) {
        match owner {
            None => self.main.push((at_ms, item)),
            Some(index) => self.agents.entry(index).or_default().items.push((at_ms, item)),
        }
    }

    fn route(&mut self, owner: Option<usize>, at_ms: u64, event: EngineEvent) {
        match event {
            EngineEvent::AgentEvent { index, event } => self.route(Some(index), at_ms, *event),
            EngineEvent::AgentSpawned { index, kind, summary, prompt } => {
                let spawn = self.agents.entry(index).or_default();
                spawn.items.insert(0, (at_ms, Item::Prompt(prompt.clone())));
                (spawn.kind, spawn.summary, spawn.prompt) = (kind, summary, prompt);
                self.push(owner, at_ms, Item::Spawned(index));
            }
            EngineEvent::AgentUpdate { index, status, .. } => {
                self.agents.entry(index).or_default().status = Some(status)
            }
            EngineEvent::Usage(usage) => {
                self.totals.add(&usage);
                self.push(owner, at_ms, Item::Event(EngineEvent::Usage(usage)));
            }
            other => self.push(owner, at_ms, Item::Event(other)),
        }
    }
}

pub fn build(records: Vec<Record>) -> Trace {
    let mut routed = Routed::default();
    let mut last = None;
    for record in records {
        match record {
            Record::Prompted { at_ms, prompt } => {
                last = Some(at_ms);
                routed.push(None, at_ms, Item::Prompt(prompt));
            }
            Record::Event { at_ms, event } => {
                last = Some(at_ms);
                routed.route(None, at_ms, event);
            }
            Record::Truncated { keep } => routed.push(None, last.unwrap_or_default(), Item::Truncated(keep)),
            Record::Restarted => {}
        }
        if routed.totals.started_ms == 0 {
            routed.totals.started_ms = last.unwrap_or_default();
        }
    }
    routed.totals.ended_ms = last.unwrap_or_default();
    let main = std::mem::take(&mut routed.main);
    Trace { lane: lane(main, &mut routed.agents), totals: routed.totals }
}

#[derive(Default)]
struct Draft {
    text: String,
    thinking: String,
}

struct Builder<'a> {
    steps: Vec<Step>,
    draft: Option<Draft>,
    group: Vec<Node>,
    since: u64,
    agents: &'a mut HashMap<usize, Spawn>,
}

fn lane(items: Vec<(u64, Item)>, agents: &mut HashMap<usize, Spawn>) -> Lane {
    let since = items.first().map_or(0, |(at, _)| *at);
    let mut builder = Builder { steps: Vec::new(), draft: None, group: Vec::new(), since, agents };
    for (at_ms, item) in items {
        builder.take(at_ms, item);
    }
    let since = builder.since;
    builder.finish_turn(since, None);
    builder.flush_group();
    Lane { steps: builder.steps }
}

fn node(at_ms: u64, failed: bool, kind: Kind) -> Node {
    Node { at_ms, until_ms: None, failed, kind }
}

impl Builder<'_> {
    fn take(&mut self, at: u64, item: Item) {
        match item {
            Item::Prompt(text) => self.single(node(at, false, Kind::Prompt { text })),
            Item::Truncated(keep) => self.single(node(at, false, Kind::Rewound { keep })),
            Item::Spawned(index) => self.spawned(at, index),
            Item::Event(event) => self.event(at, event),
        }
    }

    fn event(&mut self, at: u64, event: EngineEvent) {
        match event {
            EngineEvent::TextDelta(text) => self.drafting(|draft| draft.text.push_str(&text)),
            EngineEvent::ThinkingDelta(text) => self.drafting(|draft| draft.thinking.push_str(&text)),
            EngineEvent::Usage(usage) => self.usage(at, usage),
            EngineEvent::ToolStarted(call) => {
                if self.draft.is_some() {
                    self.finish_turn(at, None);
                }
                let tool = Kind::Tool { call, output: None, logs: Vec::new(), usage: Vec::new(), agent: None };
                self.group.push(node(at, false, tool));
            }
            EngineEvent::ToolLog { id, text } => {
                if let Some(Kind::Tool { logs, .. }) = self.tool(&id).map(|tool| &mut tool.kind) {
                    logs.push(text);
                }
            }
            EngineEvent::ToolResult { id, output: result, failed } => {
                if let Some(tool) = self.tool(&id) {
                    tool.until_ms = Some(at);
                    tool.failed = failed;
                    if let Kind::Tool { output, .. } = &mut tool.kind {
                        *output = Some(result);
                    }
                }
            }
            EngineEvent::Compacted(compaction) => self.single(node(at, false, Kind::Compacted(compaction))),
            EngineEvent::Steered(text) => self.single(node(at, false, Kind::Steered { text })),
            EngineEvent::FellBack(model) => self.single(node(at, false, Kind::FellBack { model })),
            EngineEvent::Failed(error) => self.single(node(at, true, Kind::Failed { error })),
            EngineEvent::Cancelled => self.single(node(at, false, Kind::Cancelled)),
            EngineEvent::Completed { duration_ms, cost_usd } => {
                self.single(node(at, false, Kind::Completed { duration_ms, cost_usd }))
            }
            EngineEvent::Diagnosed(problem) => self.note(at, "diagnosed", format!("{problem:?}")),
            EngineEvent::ModeChanged(mode) => self.note(at, "mode", format!("{mode:?}")),
            EngineEvent::Moved(path) => self.note(at, "moved", path.display().to_string()),
            EngineEvent::Verified(verdict) => self.note(at, "verified", format!("{verdict:?}")),
            EngineEvent::Asked(ask) => self.note(at, "asked", format!("{ask:?}")),
            _ => {}
        }
    }

    fn note(&mut self, at: u64, title: &str, text: String) {
        self.single(node(at, false, Kind::Note { title: title.into(), text }));
    }

    fn running(&self) -> bool {
        self.group.iter().any(|tool| tool.until_ms.is_none())
    }

    fn drafting(&mut self, write: impl FnOnce(&mut Draft)) {
        if !self.running() {
            self.flush_group();
        }
        write(self.draft.get_or_insert_with(Draft::default));
    }

    fn usage(&mut self, at: u64, usage: ModelUsage) {
        if self.running() {
            if let Some(Kind::Tool { usage: spent, .. }) = self.waiting_tool(false).map(|tool| &mut tool.kind) {
                spent.push(usage);
            }
            return;
        }
        self.flush_group();
        self.finish_turn(at, Some(usage));
    }

    fn tool(&mut self, id: &str) -> Option<&mut Node> {
        self.group.iter_mut().find(|tool| matches!(&tool.kind, Kind::Tool { call, .. } if call.id == id))
    }

    fn waiting_tool(&mut self, without_agent: bool) -> Option<&mut Node> {
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

    fn spawned(&mut self, at: u64, index: usize) {
        let spawn = self.agents.remove(&index).unwrap_or_default();
        let failed = spawn.status == Some(AgentStatus::Failed);
        let agent = Box::new(Agent {
            name: spawn.kind,
            summary: spawn.summary,
            prompt: spawn.prompt,
            status: spawn.status,
            lane: lane(spawn.items, self.agents),
        });
        match self.waiting_tool(true) {
            Some(Node { kind: Kind::Tool { agent: slot, .. }, .. }) => *slot = Some(agent),
            _ => self.single(node(at, failed, Kind::Agent(agent))),
        }
    }

    fn finish_turn(&mut self, at: u64, usage: Option<ModelUsage>) {
        if self.draft.is_none() && usage.is_none() {
            return;
        }
        let draft = self.draft.take().unwrap_or_default();
        let turn = Kind::Turn { text: draft.text, thinking: draft.thinking, usage };
        self.steps
            .push(Step { nodes: vec![Node { at_ms: self.since, until_ms: Some(at), failed: false, kind: turn }] });
        self.since = at;
    }

    fn flush_group(&mut self) {
        if self.group.is_empty() {
            return;
        }
        let nodes = std::mem::take(&mut self.group);
        self.since = nodes.iter().filter_map(|tool| tool.until_ms).max().unwrap_or(self.since).max(self.since);
        self.steps.push(Step { nodes });
    }

    fn single(&mut self, node: Node) {
        let at = node.at_ms;
        self.finish_turn(at, None);
        self.flush_group();
        self.steps.push(Step { nodes: vec![node] });
        self.since = at;
    }
}
