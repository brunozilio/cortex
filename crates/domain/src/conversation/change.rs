use super::{SessionStatus, Transcript};
use crate::{AskId, EngineEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Cut(usize),
    Entry(usize),
    Status,
    Ask,
    Tasks,
    Usage,
    Activity,
    Agent(usize),
    Checkpoints,
    Draft,
    Queue,
}

#[derive(Debug)]
pub(super) struct Before {
    status: SessionStatus,
    ask: Option<AskId>,
    activity: Option<String>,
}

impl Transcript {
    pub fn recording() -> Self {
        Self { record: true, ..Self::default() }
    }

    pub fn take_changes(&mut self) -> Vec<Change> {
        std::mem::take(&mut self.changes)
    }

    pub(super) fn tell(&mut self, change: Change) {
        if self.record && !self.changes.contains(&change) {
            self.changes.push(change);
        }
    }

    pub(super) fn touch(&mut self, index: usize) {
        self.tell(Change::Entry(index));
    }

    pub(super) fn cut(&mut self, from: usize) {
        self.tell(Change::Cut(from));
        for index in from..self.entries.len() {
            self.touch(index);
        }
    }

    pub(super) fn before(&self) -> Option<Before> {
        self.record.then(|| Before {
            status: self.status(),
            ask: self.ask().map(|pending| pending.ask.id.clone()),
            activity: self.activity.clone(),
        })
    }

    pub(super) fn tell_since(&mut self, before: Option<Before>, event: Option<&EngineEvent>) {
        let Some(before) = before else { return };
        if before.status != self.status() {
            self.tell(Change::Status);
        }
        if before.ask != self.ask().map(|pending| pending.ask.id.clone()) {
            self.tell(Change::Ask);
        }
        match event {
            Some(EngineEvent::Tasks(_)) => self.tell(Change::Tasks),
            Some(EngineEvent::Usage(_) | EngineEvent::Completed { .. }) => self.tell(Change::Usage),
            Some(
                EngineEvent::AgentSpawned { index, .. }
                | EngineEvent::AgentEvent { index, .. }
                | EngineEvent::AgentUpdate { index, .. },
            ) => self.tell(Change::Agent(*index)),
            _ => {}
        }
        if before.activity != self.activity {
            self.tell(Change::Activity);
        }
    }
}
