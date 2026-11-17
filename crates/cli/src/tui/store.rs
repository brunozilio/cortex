use sdk::Sdk;

pub type Change = Box<dyn FnOnce(&mut serde_json::Value) + Send>;

pub trait Store: Send + 'static {
    fn load(&self) -> Result<serde_json::Value, String>;
    fn save(&self, settings: serde_json::Value) -> Result<(), String>;
}

impl Store for Sdk {
    fn load(&self) -> Result<serde_json::Value, String> {
        self.settings().map_err(|error| error.to_string())
    }

    fn save(&self, settings: serde_json::Value) -> Result<(), String> {
        self.save_settings(settings).map_err(|error| error.to_string())
    }
}

pub fn apply(store: &impl Store, waiting: &mut Vec<Change>) {
    if waiting.is_empty() {
        return;
    }
    let mut settings = match store.load() {
        Ok(settings) => settings,
        Err(error) => {
            let kept = waiting.len();
            return tracing::warn!(%error, kept, "could not read the settings; the changes wait for the next save");
        }
    };
    if !settings.is_object() {
        settings = serde_json::json!({});
    }
    for change in waiting.drain(..) {
        change(&mut settings);
    }
    let _ = store.save(settings);
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
        stored: Arc<Mutex<serde_json::Value>>,
        failures_left: Arc<AtomicUsize>,
    }

    impl FailsOnce {
        fn holding(stored: serde_json::Value) -> Self {
            Self { stored: Arc::new(Mutex::new(stored)), failures_left: Arc::new(AtomicUsize::new(1)) }
        }

        fn stored(&self) -> serde_json::Value {
            self.stored.lock().expect("stored").clone()
        }
    }

    impl Store for FailsOnce {
        fn load(&self) -> Result<serde_json::Value, String> {
            let failing =
                self.failures_left.try_update(Ordering::SeqCst, Ordering::SeqCst, |left| left.checked_sub(1)).is_ok();
            if failing { Err("the file is busy".to_owned()) } else { Ok(self.stored()) }
        }

        fn save(&self, settings: serde_json::Value) -> Result<(), String> {
            *self.stored.lock().expect("stored") = settings;
            Ok(())
        }
    }

    #[test]
    fn a_failed_read_keeps_what_is_stored_and_saves_the_changes_with_the_next_ones() {
        let kept = json!({"mode": "plan", "history": {"/a": ["one"]}});
        let store = FailsOnce::holding(kept.clone());
        let saver = Saver::over(store.clone());
        saver.remember("model", "keyed:x");
        let deadline = Instant::now() + Duration::from_secs(120);
        while store.failures_left.load(Ordering::SeqCst) > 0 {
            assert!(Instant::now() < deadline, "the read was never tried");
            std::thread::yield_now();
        }
        assert_eq!(store.stored(), kept, "nothing is written over a document that could not be read");
        saver.remember("effort", "high");
        saver.flush();
        assert_eq!(
            store.stored(),
            json!({"mode": "plan", "history": {"/a": ["one"]}, "model": "keyed:x", "effort": "high"})
        );
    }
}
