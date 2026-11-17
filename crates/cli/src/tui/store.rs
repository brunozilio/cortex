use app::{Client, SettingsChange};

const WAITING_AT_MOST: usize = 500;

pub trait Store: Send + 'static {
    fn change(&self, changes: Vec<SettingsChange>) -> Result<(), String>;
}

impl Store for Client {
    fn change(&self, changes: Vec<SettingsChange>) -> Result<(), String> {
        self.change_settings(changes).map_err(|error| error.to_string())
    }
}

pub fn apply(store: &impl Store, waiting: &mut Vec<SettingsChange>) {
    if waiting.is_empty() {
        return;
    }
    match store.change(waiting.clone()) {
        Ok(()) => waiting.clear(),
        Err(error) => {
            let forgotten = waiting.len().saturating_sub(WAITING_AT_MOST);
            waiting.drain(..forgotten);
            let kept = waiting.len();
            tracing::warn!(%error, kept, forgotten, "could not save the settings; the changes wait for the next save");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    use serde_json::json;

    use super::*;
    use crate::tui::settings::Saver;

    #[derive(Clone)]
    struct FailsOnce {
        stored: Arc<Mutex<Vec<SettingsChange>>>,
        failures_left: Arc<AtomicUsize>,
    }

    impl FailsOnce {
        fn failing(times: usize) -> Self {
            Self { stored: Arc::default(), failures_left: Arc::new(AtomicUsize::new(times)) }
        }

        fn stored(&self) -> Vec<SettingsChange> {
            self.stored.lock().expect("stored").clone()
        }
    }

    impl Store for FailsOnce {
        fn change(&self, changes: Vec<SettingsChange>) -> Result<(), String> {
            let failing =
                self.failures_left.try_update(Ordering::SeqCst, Ordering::SeqCst, |left| left.checked_sub(1)).is_ok();
            if failing {
                return Err("the file is busy".to_owned());
            }
            self.stored.lock().expect("stored").extend(changes);
            Ok(())
        }
    }

    #[test]
    fn changes_that_could_not_be_saved_go_with_the_next_ones_in_the_order_they_were_made() {
        let store = FailsOnce::failing(1);
        let saver = Saver::over(store.clone());
        saver.remember("model", "keyed:x");
        let deadline = Instant::now() + Duration::from_secs(120);
        while store.failures_left.load(Ordering::SeqCst) > 0 {
            assert!(Instant::now() < deadline, "the save was never tried");
            std::thread::yield_now();
        }
        assert!(store.stored().is_empty(), "nothing was stored by the save that failed");
        saver.remember("effort", "high");
        saver.flush();
        assert_eq!(
            store.stored(),
            [SettingsChange::set(["model"], json!("keyed:x")), SettingsChange::set(["effort"], json!("high"))]
        );
    }

    #[test]
    fn changes_that_never_save_do_not_pile_up_without_end() {
        let store = FailsOnce::failing(usize::MAX);
        let mut waiting: Vec<SettingsChange> =
            (0..WAITING_AT_MOST + 20).map(|at| SettingsChange::set(["last"], json!(at))).collect();
        apply(&store, &mut waiting);
        assert_eq!(waiting.len(), WAITING_AT_MOST);
        assert_eq!(waiting.last(), Some(&SettingsChange::set(["last"], json!(WAITING_AT_MOST + 19))), "newest kept");
    }
}
