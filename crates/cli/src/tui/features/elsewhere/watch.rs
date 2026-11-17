use std::time::{Duration, Instant};

use domain::SessionId;
use domain::conversation::Transcript;
use domain::trace::Record;

use crate::tui::every::Every;

pub(super) const EVERY: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Look {
    Nothing,
    Settle(SessionId),
    Check(SessionId, u64),
}

#[derive(Debug)]
pub(super) struct Watch {
    session: Option<SessionId>,
    settled: bool,
    length: u64,
    seen: usize,
    every: Every,
}

impl Default for Watch {
    fn default() -> Self {
        Self { session: None, settled: false, length: 0, seen: 0, every: Every::new(EVERY) }
    }
}

impl Watch {
    pub(super) fn look(&mut self, session: Option<&SessionId>, busy: bool, now: Instant) -> Look {
        let Some(session) = session else {
            *self = Self::default();
            return Look::Nothing;
        };
        if busy || self.session.as_ref() != Some(session) {
            (self.session, self.settled) = (Some(session.clone()), false);
        }
        if busy || !self.every.due(now) {
            return Look::Nothing;
        }
        if self.settled { Look::Check(session.clone(), self.length) } else { Look::Settle(session.clone()) }
    }

    pub(super) fn settled(&mut self, session: &SessionId, length: u64, records: usize) {
        self.every.landed();
        if self.session.as_ref() == Some(session) {
            (self.settled, self.length, self.seen) = (true, length, records);
        }
    }

    pub(super) fn arrived(
        &mut self,
        transcript: &mut Transcript,
        session: &SessionId,
        length: u64,
        records: &[Record],
        busy: bool,
    ) {
        self.every.landed();
        if busy || !self.settled || self.session.as_ref() != Some(session) || records.len() < self.seen {
            self.settled = false;
            return;
        }
        self.seen += transcript.follow(&records[self.seen..]);
        self.length = length;
    }

    pub(super) fn unchanged(&mut self) {
        self.every.landed();
    }
}

#[cfg(test)]
#[path = "watch_tests.rs"]
mod tests;
