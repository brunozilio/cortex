use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

use agent_engine::RunHandle;
use domain::RunId;
use platform::sync::Locked;

enum Slot {
    Active(Box<dyn RunHandle>),
    Cancelling,
    Finished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Reach {
    Reached,
    Inactive,
}

#[derive(Default)]
pub(crate) struct RunTable {
    slots: Mutex<HashMap<RunId, Slot>>,
}

impl std::fmt::Debug for RunTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunTable").field("runs", &self.lock().len()).finish()
    }
}

impl RunTable {
    pub fn is_active(&self, run: RunId) -> bool {
        matches!(self.lock().get(&run), Some(Slot::Active(_) | Slot::Cancelling))
    }

    pub fn insert(&self, run: RunId, handle: Box<dyn RunHandle>) {
        let mut slots = self.lock();
        if matches!(slots.get(&run), Some(Slot::Finished)) {
            slots.remove(&run);
        } else {
            slots.insert(run, Slot::Active(handle));
        }
    }

    pub fn finish(&self, run: RunId) {
        let mut slots = self.lock();
        match slots.get(&run) {
            Some(Slot::Active(_) | Slot::Cancelling) => {
                slots.remove(&run);
            }
            Some(Slot::Finished) => {}
            None => {
                slots.insert(run, Slot::Finished);
            }
        }
    }

    pub fn cancel(&self, run: RunId) -> Reach {
        let mut handle = {
            let mut slots = self.lock();
            match slots.remove(&run) {
                Some(Slot::Active(handle)) => {
                    slots.insert(run, Slot::Cancelling);
                    handle
                }
                Some(other) => {
                    slots.insert(run, other);
                    return Reach::Inactive;
                }
                None => return Reach::Inactive,
            }
        };
        handle.cancel();
        Reach::Reached
    }

    pub fn control(&self, run: RunId, control: impl FnOnce(&mut dyn RunHandle)) -> Reach {
        match self.lock().get_mut(&run) {
            Some(Slot::Active(handle)) => {
                control(handle.as_mut());
                Reach::Reached
            }
            _ => Reach::Inactive,
        }
    }

    pub fn cancel_all(&self) {
        let runs: Vec<RunId> = self.lock().keys().copied().collect();
        for run in runs {
            self.cancel(run);
        }
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<RunId, Slot>> {
        self.slots.locked()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    struct CountingHandle(Arc<AtomicUsize>);

    impl RunHandle for CountingHandle {
        fn cancel(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn handle() -> (Box<dyn RunHandle>, Arc<AtomicUsize>) {
        let count = Arc::new(AtomicUsize::new(0));
        (Box::new(CountingHandle(Arc::clone(&count))), count)
    }

    #[test]
    fn normal_lifecycle() {
        let table = RunTable::default();
        let run = RunId::new();
        let (h, _) = handle();
        table.insert(run, h);
        assert!(table.is_active(run));
        table.finish(run);
        assert!(!table.is_active(run));
        assert!(table.lock().is_empty());
    }

    #[test]
    fn finish_before_insert_leaves_nothing_behind() {
        let table = RunTable::default();
        let run = RunId::new();
        table.finish(run);
        table.finish(run);
        let (h, _) = handle();
        table.insert(run, h);
        assert!(!table.is_active(run));
        assert!(table.lock().is_empty());
    }

    #[test]
    fn cancel_calls_handle_once_and_clears_on_finish() {
        let table = RunTable::default();
        let run = RunId::new();
        let (h, count) = handle();
        table.insert(run, h);
        assert_eq!(table.cancel(run), Reach::Reached);
        assert_eq!(table.cancel(run), Reach::Inactive);
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert!(table.is_active(run));
        table.finish(run);
        assert!(table.lock().is_empty());
        assert_eq!(table.cancel(RunId::new()), Reach::Inactive);
    }

    #[test]
    fn control_reaches_only_active_runs() {
        let table = RunTable::default();
        let run = RunId::new();
        let (h, _) = handle();
        table.insert(run, h);
        let mut reached = 0;
        assert_eq!(table.control(run, |_| reached += 1), Reach::Reached);
        table.cancel(run);
        assert_eq!(table.control(run, |_| reached += 1), Reach::Inactive);
        assert_eq!(table.control(RunId::new(), |_| reached += 1), Reach::Inactive);
        assert_eq!(reached, 1);
    }

    #[test]
    fn cancel_all_cancels_every_active_run() {
        let table = RunTable::default();
        let (a, count_a) = handle();
        let (b, count_b) = handle();
        table.insert(RunId::new(), a);
        table.insert(RunId::new(), b);
        table.cancel_all();
        assert_eq!(count_a.load(Ordering::SeqCst) + count_b.load(Ordering::SeqCst), 2);
    }
}
