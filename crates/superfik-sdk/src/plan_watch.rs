//! A planning run's text, read as it streams so that the finished plan's
//! `Verification` section becomes the session's criteria.

use std::sync::{Arc, Mutex, PoisonError};

use superfik_core::{EngineEvent, Verifications};

#[derive(Debug)]
pub(crate) struct PlanWatch {
    verifications: Arc<Mutex<Verifications>>,
    text: String,
}

impl PlanWatch {
    pub(crate) fn new(verifications: Arc<Mutex<Verifications>>) -> Self {
        Self { verifications, text: String::new() }
    }

    pub(crate) fn observe(&mut self, event: &EngineEvent) {
        match event {
            EngineEvent::TextDelta(delta) => self.text.push_str(delta),
            EngineEvent::Completed { .. } => {
                self.verifications.lock().unwrap_or_else(PoisonError::into_inner).plan_finished(&self.text);
            }
            _ => {}
        }
    }
}
