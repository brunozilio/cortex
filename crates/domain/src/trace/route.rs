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
        let at = |at_ms: u64, last: &mut Option<u64>| {
            *last = Some(at_ms);
            at_ms
        };
        match record {
            Record::Prompted { at_ms, prompt } => routed.push(None, at(at_ms, &mut last), Item::Prompt(prompt)),
            Record::Event { at_ms, event } => routed.route(None, at(at_ms, &mut last), event),
            Record::Called { at_ms, agent, call } => routed.push(agent, at(at_ms, &mut last), Item::Called(call)),
            Record::Opened { at_ms, run } => routed.push(None, at(at_ms, &mut last), Item::Opened(run)),
            Record::Truncated { keep } => routed.push(None, last.unwrap_or_default(), Item::Truncated(keep)),
            Record::Partial => routed.push(None, last.unwrap_or_default(), Item::Partial),
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
