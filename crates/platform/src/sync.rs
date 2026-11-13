use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};

pub trait Locked<T: ?Sized> {
    fn locked(&self) -> MutexGuard<'_, T>;
}

impl<T: ?Sized> Locked<T> for Mutex<T> {
    fn locked(&self) -> MutexGuard<'_, T> {
        self.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

pub fn wait<'a, T>(ready: &Condvar, guard: MutexGuard<'a, T>) -> MutexGuard<'a, T> {
    ready.wait(guard).unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn poisoned(value: u32) -> Arc<Mutex<u32>> {
        let shared = Arc::new(Mutex::new(value));
        let held = Arc::clone(&shared);
        let crashed = std::thread::spawn(move || {
            let _guard = held.lock();
            panic!("poison the lock");
        })
        .join();
        assert!(crashed.is_err());
        assert!(shared.is_poisoned());
        shared
    }

    #[test]
    fn a_poisoned_lock_still_hands_out_its_value() {
        let shared = poisoned(7);
        *shared.locked() += 1;
        assert_eq!(*shared.locked(), 8);
    }

    #[test]
    fn waiting_returns_the_guard_once_woken() {
        let pair = Arc::new((Mutex::new(false), Condvar::new()));
        let waker = Arc::clone(&pair);
        let woken = std::thread::spawn(move || {
            *waker.0.locked() = true;
            waker.1.notify_all();
        });
        let mut ready = pair.0.locked();
        while !*ready {
            ready = wait(&pair.1, ready);
        }
        assert!(woken.join().is_ok());
        assert!(*ready);
    }
}
