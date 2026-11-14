use sdk::RunControl;

use super::rewind::Checkpoint;
use super::stopwatch::Stopwatch;

#[derive(Default)]
pub struct Turn {
    pub run: Option<RunControl>,
    pub started: Option<Stopwatch>,
    pub queued: Vec<String>,
    pub(super) retrying: bool,
    pub(super) transient: bool,
    pub(super) auto_retried: bool,
    pub auto_retry: Option<std::time::Instant>,
    pub(super) rewound: Option<domain::Rewound>,
    pub checkpoints: Vec<Checkpoint>,
}

impl Turn {
    pub fn busy(&self) -> bool {
        self.run.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_turn_is_busy_only_while_a_run_goes_on() {
        let idle = Turn::default();
        assert!(!idle.busy());
        assert!(idle.queued.is_empty() && idle.checkpoints.is_empty() && !idle.retrying);
    }
}
