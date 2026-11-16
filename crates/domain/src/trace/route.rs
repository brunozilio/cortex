use std::collections::HashMap;

use super::lane::lane;
use super::{ModelCall, Record, RunInfo, Totals, Trace};
use crate::{AgentStatus, EngineEvent};

pub(super) enum Item {
    Prompt(String),
    Event(EngineEvent),
    Truncated(usize),
    Spawned(usize),
    Called(ModelCall),
    Opened(RunInfo),
    Partial,
}

#[derive(Default)]
pub(super) struct Spawn {
    pub(super) kind: String,
    pub(super) summary: String,
    pub(super) prompt: String,
    pub(super) status: Option<AgentStatus>,
    pub(super) items: Vec<(u64, Item)>,
}

#[derive(Default)]
struct Routed {
    main: Vec<(u64, Item)>,
    agents: HashMap<usize, Spawn>,
    live: HashMap<usize, Lineage>,
    next: usize,
    totals: Totals,
}

#[derive(Clone, Copy)]
struct Lineage {
    key: usize,
    spawned: bool,
}

impl Routed {
    fn push(&mut self, owner: Option<usize>, at_ms: u64, item: Item) {
        match owner {
            None => self.main.push((at_ms, item)),
            Some(key) => self.agents.entry(key).or_default().items.push((at_ms, item)),
        }
    }

    fn fresh(&mut self, spawned: bool) -> Lineage {
        let lineage = Lineage { key: self.next, spawned };
        self.next += 1;
        lineage
    }

    fn key(&mut self, index: usize) -> usize {
        match self.live.get(&index) {
            Some(lineage) => lineage.key,
            None => {
                let lineage = self.fresh(false);
                self.live.insert(index, lineage);
                lineage.key
            }
        }
    }

    fn spawn_key(&mut self, index: usize) -> usize {
        let lineage = match self.live.get(&index) {
            Some(lineage) if !lineage.spawned => Lineage { spawned: true, ..*lineage },
            _ => self.fresh(true),
        };
        self.live.insert(index, lineage);
        lineage.key
    }

    fn route(&mut self, owner: Option<usize>, at_ms: u64, event: EngineEvent) {
        match event {
            EngineEvent::AgentEvent { index, event } => {
                let key = self.key(index);
                self.route(Some(key), at_ms, *event)
            }
            EngineEvent::AgentSpawned { index, kind, summary, prompt } => {
                let key = self.spawn_key(index);
                let spawn = self.agents.entry(key).or_default();
                spawn.items.insert(0, (at_ms, Item::Prompt(prompt.clone())));
                (spawn.kind, spawn.summary, spawn.prompt) = (kind, summary, prompt);
                self.push(owner, at_ms, Item::Spawned(key));
            }
            EngineEvent::AgentUpdate { index, status, .. } => {
                let key = self.key(index);
                self.agents.entry(key).or_default().status = Some(status)
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
        let at = |at_ms: u64, last: &mut Option<u64>| {
            *last = Some(at_ms);
            at_ms
        };
        match record {
            Record::Prompted { at_ms, prompt } => routed.push(None, at(at_ms, &mut last), Item::Prompt(prompt)),
            Record::Event { at_ms, event } => routed.route(None, at(at_ms, &mut last), event),
            Record::Called { at_ms, agent, call } => {
                let owner = agent.map(|index| routed.key(index));
                routed.push(owner, at(at_ms, &mut last), Item::Called(call))
            }
            Record::Opened { at_ms, run } => routed.push(None, at(at_ms, &mut last), Item::Opened(run)),
            Record::Truncated { keep } => routed.push(None, last.unwrap_or_default(), Item::Truncated(keep)),
            Record::Partial => routed.push(None, last.unwrap_or_default(), Item::Partial),
            Record::Restarted | Record::Shown { .. } => {}
        }
        if routed.totals.started_ms == 0 {
            routed.totals.started_ms = last.unwrap_or_default();
        }
    }
    routed.totals.ended_ms = last.unwrap_or_default();
    let main = std::mem::take(&mut routed.main);
    Trace { lane: lane(main, &mut routed.agents), totals: routed.totals }
}
