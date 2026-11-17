use tokio::sync::{Mutex, MutexGuard};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Default)]
pub(super) struct Turns {
    busy: Mutex<()>,
}

#[derive(Debug)]
pub(super) struct Turn<'a> {
    _held: MutexGuard<'a, ()>,
}

impl Turns {
    pub(super) fn take(&self) -> Turn<'_> {
        Turn { _held: self.busy.blocking_lock() }
    }

    pub(super) async fn take_unless(&self, cancel: &CancellationToken) -> Option<Turn<'_>> {
        tokio::select! {
            biased;
            () = cancel.cancelled() => None,
            held = self.busy.lock() => Some(Turn { _held: held }),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;

    #[test]
    fn a_waiting_turn_starts_as_soon_as_the_last_one_ends() {
        let turns = Turns::default();
        let never = CancellationToken::new();
        let first = turns.take();
        std::thread::scope(|scope| {
            let waiter = scope.spawn(|| {
                let _second = platform::runtime::block_on(turns.take_unless(&never)).expect("runtime").expect("second");
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
    fn a_cancelled_wait_gives_up_its_turn_at_once() {
        let turns = Turns::default();
        let _held = turns.take();
        let cancel = CancellationToken::new();
        let canceller = cancel.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(20));
            canceller.cancel();
        });
        let started = Instant::now();
        assert!(platform::runtime::block_on(turns.take_unless(&cancel)).expect("runtime").is_none());
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}
