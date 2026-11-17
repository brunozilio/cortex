use std::ops::Deref;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio_util::sync::CancellationToken;

#[derive(Debug, Default)]
pub struct Stop {
    flag: Arc<AtomicBool>,
    token: CancellationToken,
}

impl Stop {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn stopped() -> Self {
        let stop = Self::default();
        stop.cancel();
        stop
    }

    pub fn cancel(&self) {
        self.flag.store(true, Ordering::SeqCst);
        self.token.cancel();
    }

    pub fn is_stopped(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }

    pub fn token(&self) -> &CancellationToken {
        &self.token
    }

    pub fn flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.flag)
    }

    pub async fn wait(&self) {
        self.token.cancelled().await;
    }

    pub fn load(&self, _order: Ordering) -> bool {
        self.is_stopped()
    }

    pub fn store(&self, stopped: bool, order: Ordering) {
        if stopped {
            self.cancel();
        } else {
            self.flag.store(false, order);
        }
    }
}

impl Deref for Stop {
    type Target = AtomicBool;

    fn deref(&self) -> &AtomicBool {
        &self.flag
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    #[test]
    fn stopping_sets_the_flag_and_wakes_whoever_waits() {
        let stop = Arc::new(Stop::new());
        let stopper = Arc::clone(&stop);
        let runtime = crate::runtime::shared().expect("runtime");
        let started = Instant::now();
        let waited = runtime.block_on(async move {
            let waiter = tokio::spawn(async move { stopper.wait().await });
            tokio::time::sleep(Duration::from_millis(20)).await;
            stop.store(true, Ordering::Relaxed);
            waiter.await.is_ok() && stop.is_stopped()
        });
        assert!(waited);
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn code_that_reads_a_flag_sees_the_stop() {
        fn polled(flag: &AtomicBool) -> bool {
            flag.load(Ordering::Relaxed)
        }
        let stop = Stop::new();
        assert!(!polled(&stop));
        stop.cancel();
        assert!(polled(&stop));
        assert!(Stop::stopped().token().is_cancelled());
    }
}
