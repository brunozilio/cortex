use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use domain::watchdog::{Stall, Watchdog};
use platform::sync::Locked;

const LOOKS_EVERY: Duration = Duration::from_millis(500);

#[derive(Clone)]
pub struct Dog {
    started: Instant,
    state: Arc<Mutex<Watchdog>>,
}

impl Dog {
    pub fn start() -> Self {
        let dog = Self { started: Instant::now(), state: Arc::default() };
        let watcher = dog.clone();
        let spawned = std::thread::Builder::new().name("watchdog".into()).spawn(move || {
            while Arc::strong_count(&watcher.state) > 1 {
                std::thread::sleep(LOOKS_EVERY);
                if let Some(stall) = watcher.state.locked().check(watcher.now()) {
                    tracing::error!(step = %stall.step, millis = stall.for_ms, "{}", stall.describe());
                }
            }
        });
        if let Err(error) = spawned {
            tracing::warn!(%error, "the watchdog did not start");
        }
        dog
    }

    pub fn enter(&self, step: &str) -> Option<Stall> {
        self.state.locked().enter(self.now(), step)
    }

    #[cfg(test)]
    pub fn step(&self) -> String {
        self.state.locked().step().to_owned()
    }

    pub fn pause(&self) {
        self.state.locked().pause();
    }

    fn now(&self) -> u64 {
        u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
}
