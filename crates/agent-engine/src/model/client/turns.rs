use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};

use platform::sync::Locked;
use std::time::Duration;

const STOP_CHECK: Duration = Duration::from_millis(25);

#[derive(Debug, Default)]
pub(super) struct Turns {
    busy: Mutex<bool>,
    freed: Condvar,
}

#[derive(Debug)]
pub(super) struct Turn<'a>(&'a Turns);

impl Turns {
    fn busy(&self) -> MutexGuard<'_, bool> {
        self.busy.locked()
    }

    pub(super) fn take(&self) -> Turn<'_> {
        let mut busy = self.busy();
        while *busy {
            busy = platform::sync::wait(&self.freed, busy);
        }
        *busy = true;
        Turn(self)
    }

    pub(super) fn take_unless(&self, stop: &AtomicBool) -> Option<Turn<'_>> {
        let mut busy = self.busy();
        loop {
            if stop.load(Ordering::Relaxed) {
                return None;
            }
            if !*busy {
                *busy = true;
                return Some(Turn(self));
            }
            busy = self.freed.wait_timeout(busy, STOP_CHECK).unwrap_or_else(PoisonError::into_inner).0;
        }
    }
}

impl Drop for Turn<'_> {
    fn drop(&mut self) {
        *self.0.busy() = false;
        self.0.freed.notify_one();
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;

    #[test]
    fn a_waiting_turn_starts_as_soon_as_the_last_one_ends() {
        let turns = Turns::default();
        let never = AtomicBool::new(false);
        let first = turns.take();
        std::thread::scope(|scope| {
            let waiter = scope.spawn(|| {
                let _second = turns.take_unless(&never).expect("second");
                Instant::now()
            });
            std::thread::sleep(Duration::from_millis(50));
            let released = Instant::now();
            drop(first);
            let started = waiter.join().expect("waiter");
            assert!(started >= released);
            assert!(started.duration_since(released) < Duration::from_millis(20));
        });
    }

    #[test]
    fn a_stopped_wait_gives_up_its_turn() {
        let turns = Turns::default();
        let _held = turns.take();
        let stop = AtomicBool::new(true);
        assert!(turns.take_unless(&stop).is_none());
    }
}
