use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stopwatch {
    since: Instant,
    idle: Duration,
    paused_at: Option<Instant>,
}

impl Stopwatch {
    pub fn start(now: Instant) -> Self {
        Self { since: now, idle: Duration::ZERO, paused_at: None }
    }

    pub fn pause(&mut self, now: Instant) {
        self.paused_at.get_or_insert(now);
    }

    pub fn resume(&mut self, now: Instant) {
        if let Some(at) = self.paused_at.take() {
            self.idle += now.saturating_duration_since(at);
        }
    }

    #[cfg(test)]
    pub fn is_paused(&self) -> bool {
        self.paused_at.is_some()
    }

    pub fn elapsed(&self, now: Instant) -> Duration {
        let until = self.paused_at.unwrap_or(now);
        until.saturating_duration_since(self.since).saturating_sub(self.idle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secs(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    #[test]
    fn a_running_stopwatch_counts_wall_time() {
        let t0 = Instant::now();
        let watch = Stopwatch::start(t0);
        assert_eq!(watch.elapsed(t0 + secs(7)), secs(7));
    }

    #[test]
    fn time_spent_paused_is_left_out() {
        let t0 = Instant::now();
        let mut watch = Stopwatch::start(t0);
        watch.pause(t0 + secs(3));
        assert!(watch.is_paused());
        assert_eq!(watch.elapsed(t0 + secs(60)), secs(3));
        watch.resume(t0 + secs(60));
        assert!(!watch.is_paused());
        assert_eq!(watch.elapsed(t0 + secs(62)), secs(5));
    }

    #[test]
    fn pausing_twice_keeps_the_first_pause_and_resuming_twice_is_harmless() {
        let t0 = Instant::now();
        let mut watch = Stopwatch::start(t0);
        watch.pause(t0 + secs(2));
        watch.pause(t0 + secs(10));
        watch.resume(t0 + secs(20));
        watch.resume(t0 + secs(30));
        assert_eq!(watch.elapsed(t0 + secs(31)), secs(13));
    }
}
