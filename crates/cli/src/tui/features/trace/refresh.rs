use std::time::{Duration, Instant};

pub(super) const GAP: Duration = Duration::from_secs(5);

#[derive(Debug, Default)]
pub(super) struct Refresh {
    due: bool,
    busy: bool,
    last: Option<Instant>,
}

impl Refresh {
    pub(super) fn changed(&mut self) {
        self.due = true;
    }

    pub(super) fn start(&mut self, now: Instant) -> bool {
        let early = self.last.is_some_and(|at| now.saturating_duration_since(at) < GAP);
        if !self.due || self.busy || early {
            return false;
        }
        (self.due, self.busy, self.last) = (false, true, Some(now));
        true
    }

    pub(super) fn done(&mut self) {
        self.busy = false;
    }
}

#[cfg(test)]
#[path = "refresh_tests.rs"]
mod tests;
