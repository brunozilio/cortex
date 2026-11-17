use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Every {
    period: Duration,
    last: Option<Instant>,
    in_flight: bool,
}

impl Every {
    pub const fn new(period: Duration) -> Self {
        Self { period, last: None, in_flight: false }
    }

    pub const fn second() -> Self {
        Self::new(Duration::from_secs(1))
    }

    pub fn due(&mut self, now: Instant) -> bool {
        let early = self.last.is_some_and(|at| now.saturating_duration_since(at) < self.period);
        if self.in_flight || early {
            return false;
        }
        (self.in_flight, self.last) = (true, Some(now));
        true
    }

    pub fn landed(&mut self) {
        self.in_flight = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PERIOD: Duration = Duration::from_secs(1);

    #[test]
    fn the_first_look_is_due_and_the_next_waits_for_the_period() {
        let start = Instant::now();
        let mut every = Every::new(PERIOD);
        assert!(every.due(start));
        every.landed();
        assert!(!every.due(start + Duration::from_millis(999)), "too soon");
        assert!(every.due(start + PERIOD));
    }

    #[test]
    fn nothing_is_due_while_the_one_before_is_in_flight_however_long_it_takes() {
        let start = Instant::now();
        let mut every = Every::new(PERIOD);
        assert!(every.due(start));
        assert!(!every.due(start + PERIOD * 10), "the one before has not landed");
        every.landed();
        assert!(every.due(start + PERIOD * 10), "the period counts from when it started, so a slow one is due at once");
    }

    #[test]
    fn a_clock_that_goes_back_does_not_make_it_due() {
        let start = Instant::now() + PERIOD;
        let mut every = Every::second();
        assert!(every.due(start));
        every.landed();
        assert!(!every.due(start - Duration::from_millis(500)));
    }
}
