use std::cell::Cell;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crossbeam_channel::{Receiver, Sender};

pub const MOST: usize = 8;

type Job = Box<dyn FnOnce() + Send>;

pub struct Workers {
    most: usize,
    jobs: Sender<Job>,
    taken: Receiver<Job>,
    unfinished: Arc<AtomicUsize>,
    started: Cell<usize>,
}

impl std::fmt::Debug for Workers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Workers").field("most", &self.most).field("started", &self.started.get()).finish()
    }
}

impl Default for Workers {
    fn default() -> Self {
        Self::at_most(MOST)
    }
}

impl Workers {
    pub fn at_most(most: usize) -> Self {
        let (jobs, taken) = crossbeam_channel::unbounded();
        Self { most: most.max(1), jobs, taken, unfinished: Arc::default(), started: Cell::new(0) }
    }

    pub fn run(&self, job: impl FnOnce() + Send + 'static) {
        let unfinished = self.unfinished.fetch_add(1, Ordering::SeqCst) + 1;
        if unfinished > self.started.get() && self.started.get() < self.most {
            self.start_one();
        }
        if self.started.get() == 0 {
            self.unfinished.fetch_sub(1, Ordering::SeqCst);
            return job();
        }
        let _ = self.jobs.send(Box::new(job));
    }

    fn start_one(&self) {
        let (taken, unfinished) = (self.taken.clone(), Arc::clone(&self.unfinished));
        let name = format!("tui-worker-{}", self.started.get());
        let spawned = std::thread::Builder::new().name(name).spawn(move || {
            for job in taken {
                if std::panic::catch_unwind(std::panic::AssertUnwindSafe(job)).is_err() {
                    tracing::error!("a background task panicked");
                }
                unfinished.fetch_sub(1, Ordering::SeqCst);
            }
        });
        match spawned {
            Ok(_) => self.started.set(self.started.get() + 1),
            Err(error) => tracing::warn!(%error, "a worker did not start"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    const SOON: Duration = Duration::from_secs(30);

    #[test]
    fn no_more_than_the_most_run_at_once_and_the_rest_wait_their_turn() {
        let workers = Workers::at_most(3);
        let (running, peak) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
        let (release, held) = crossbeam_channel::unbounded::<()>();
        let (finished, done) = crossbeam_channel::unbounded();
        for number in 0..10 {
            let (running, peak, held, finished) = (running.clone(), peak.clone(), held.clone(), finished.clone());
            workers.run(move || {
                let now = running.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(now, Ordering::SeqCst);
                let _ = held.recv();
                running.fetch_sub(1, Ordering::SeqCst);
                let _ = finished.send(number);
            });
        }
        assert_eq!(workers.started.get(), 3, "three threads for ten tasks");
        for _ in 0..10 {
            release.send(()).expect("release");
        }
        let mut numbers: Vec<i32> = (0..10).map(|_| done.recv_timeout(SOON).expect("every task runs")).collect();
        numbers.sort_unstable();
        assert_eq!(numbers, (0..10).collect::<Vec<_>>());
        assert!(peak.load(Ordering::SeqCst) <= 3, "{peak:?}");
    }

    #[test]
    fn one_task_at_a_time_starts_one_thread_only() {
        let workers = Workers::at_most(8);
        let (finished, done) = crossbeam_channel::unbounded();
        for number in 0..5 {
            let finished = finished.clone();
            workers.run(move || {
                let _ = finished.send(number);
            });
            assert_eq!(done.recv_timeout(SOON), Ok(number));
            while workers.unfinished.load(Ordering::SeqCst) > 0 {
                std::thread::yield_now();
            }
        }
        assert_eq!(workers.started.get(), 1);
    }
}
