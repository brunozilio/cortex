use std::time::{Duration, Instant};

use domain::SessionId;
use domain::conversation::Transcript;
use domain::trace::Record;

pub(super) const EVERY: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Look {
    Nothing,
    Settle(SessionId),
    Check(SessionId, u64),
}

#[derive(Debug, Default)]
pub(super) struct Watch {
    session: Option<SessionId>,
    settled: bool,
    length: u64,
    seen: usize,
    asking: bool,
    last: Option<Instant>,
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
        let early = self.last.is_some_and(|at| now.saturating_duration_since(at) < EVERY);
        if busy || self.asking || early {
            return Look::Nothing;
        }
        (self.asking, self.last) = (true, Some(now));
        if self.settled { Look::Check(session.clone(), self.length) } else { Look::Settle(session.clone()) }
    }

    pub(super) fn settled(&mut self, session: &SessionId, length: u64, records: usize) {
        self.asking = false;
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
        self.asking = false;
        if busy || !self.settled || self.session.as_ref() != Some(session) || records.len() < self.seen {
            self.settled = false;
            return;
        }
        self.seen += transcript.follow(&records[self.seen..]);
        self.length = length;
    }

    pub(super) fn unchanged(&mut self) {
        self.asking = false;
    }
}

#[cfg(test)]
#[path = "watch_tests.rs"]
mod tests;
