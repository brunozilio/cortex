//! Bookkeeping of in-flight runs, so they can be cancelled or answered,
//! by whoever started them only: a run's id is no key to it.
//!
//! An engine may finish a run before `start` even returns its handle, so
//! the table copes with "finished" arriving before "inserted".

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

use superfik_core::{Principal, RunId};
use superfik_engine::RunHandle;

enum Slot {
    Active(Principal, Box<dyn RunHandle>),
    Cancelling(Principal),
    Finished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Reach {
    Reached,
    Inactive,
    NotOwner,
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
        matches!(self.lock().get(&run), Some(Slot::Active(..) | Slot::Cancelling(_)))
    }

    pub fn insert(&self, run: RunId, owner: Principal, handle: Box<dyn RunHandle>) {
        let mut slots = self.lock();
        if matches!(slots.get(&run), Some(Slot::Finished)) {
            slots.remove(&run);
        } else {
            slots.insert(run, Slot::Active(owner, handle));
        }
    }

    pub fn finish(&self, run: RunId) {
        let mut slots = self.lock();
        match slots.get(&run) {
            Some(Slot::Active(..) | Slot::Cancelling(_)) => {
                slots.remove(&run);
            }
            Some(Slot::Finished) => {}
            None => {
                slots.insert(run, Slot::Finished);
            }
        }
    }

    pub fn cancel(&self, run: RunId, by: &Principal) -> Reach {
        self.cancel_as(run, Some(by))
    }

    fn cancel_as(&self, run: RunId, by: Option<&Principal>) -> Reach {
        let mut handle = {
            let mut slots = self.lock();
            match slots.remove(&run) {
                Some(Slot::Active(owner, handle)) if by.is_none_or(|by| *by == owner) => {
                    slots.insert(run, Slot::Cancelling(owner));
                    handle
                }
                Some(other) => {
                    let reach = match &other {
                        Slot::Active(owner, _) | Slot::Cancelling(owner) if by.is_some_and(|by| by != owner) => {
                            Reach::NotOwner
                        }
                        _ => Reach::Inactive,
                    };
                    slots.insert(run, other);
                    return reach;
                }
                None => return Reach::Inactive,
            }
        };
        handle.cancel();
        Reach::Reached
    }

    pub fn control(&self, run: RunId, by: &Principal, control: impl FnOnce(&mut dyn RunHandle)) -> Reach {
        match self.lock().get_mut(&run) {
            Some(Slot::Active(owner, _) | Slot::Cancelling(owner)) if owner != by => Reach::NotOwner,
            Some(Slot::Active(_, handle)) => {
                control(handle.as_mut());
                Reach::Reached
            }
            _ => Reach::Inactive,
        }
    }

    pub fn cancel_all(&self) {
        let runs: Vec<RunId> = self.lock().keys().copied().collect();
        for run in runs {
            self.cancel_as(run, None);
        }
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<RunId, Slot>> {
        self.slots.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use superfik_core::{OrgId, UserId};

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

    fn owner() -> Principal {
        Principal::LocalOwner
    }

    #[test]
    fn normal_lifecycle() {
        let table = RunTable::default();
        let run = RunId::new();
        let (h, _) = handle();
        table.insert(run, owner(), h);
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
        table.insert(run, owner(), h);
        assert!(!table.is_active(run));
        assert!(table.lock().is_empty());
    }

    #[test]
    fn cancel_calls_handle_once_and_clears_on_finish() {
        let table = RunTable::default();
        let run = RunId::new();
        let (h, count) = handle();
        table.insert(run, owner(), h);
        assert_eq!(table.cancel(run, &owner()), Reach::Reached);
        assert_eq!(table.cancel(run, &owner()), Reach::Inactive);
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert!(table.is_active(run));
        table.finish(run);
        assert!(table.lock().is_empty());
        assert_eq!(table.cancel(RunId::new(), &owner()), Reach::Inactive);
    }

    #[test]
    fn control_reaches_only_active_runs() {
        let table = RunTable::default();
        let run = RunId::new();
        let (h, _) = handle();
        table.insert(run, owner(), h);
        let mut reached = 0;
        assert_eq!(table.control(run, &owner(), |_| reached += 1), Reach::Reached);
        table.cancel(run, &owner());
        assert_eq!(table.control(run, &owner(), |_| reached += 1), Reach::Inactive);
        assert_eq!(table.control(RunId::new(), &owner(), |_| reached += 1), Reach::Inactive);
        assert_eq!(reached, 1);
    }

    #[test]
    fn only_the_runs_owner_controls_or_cancels_it() {
        let table = RunTable::default();
        let run = RunId::new();
        let (h, count) = handle();
        table.insert(run, owner(), h);
        let stranger = Principal::member(UserId::new(), OrgId::new());
        let mut reached = 0;
        assert_eq!(table.control(run, &stranger, |_| reached += 1), Reach::NotOwner);
        assert_eq!(table.cancel(run, &stranger), Reach::NotOwner);
        assert_eq!((reached, count.load(Ordering::SeqCst)), (0, 0));
        assert!(table.is_active(run));
        assert_eq!(table.control(run, &owner(), |_| reached += 1), Reach::Reached);
        assert_eq!(table.cancel(run, &owner()), Reach::Reached);
        assert_eq!((reached, count.load(Ordering::SeqCst)), (1, 1));
    }

    #[test]
    fn cancel_all_cancels_every_active_run() {
        let table = RunTable::default();
        let (a, count_a) = handle();
        let (b, count_b) = handle();
        table.insert(RunId::new(), owner(), a);
        table.insert(RunId::new(), Principal::member(UserId::new(), OrgId::new()), b);
        table.cancel_all();
        assert_eq!(count_a.load(Ordering::SeqCst) + count_b.load(Ordering::SeqCst), 2);
    }
}
