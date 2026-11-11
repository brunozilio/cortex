use std::sync::{Arc, Mutex, PoisonError};

use domain::{AskKind, EngineEvent, PermissionMode, Verifications};

#[derive(Debug)]
pub(crate) struct PlanWatch {
    verifications: Arc<Mutex<Verifications>>,
    planning: bool,
    text: String,
    presented: Option<String>,
}

impl PlanWatch {
    pub(crate) fn new(verifications: Arc<Mutex<Verifications>>, planning: bool) -> Self {
        Self { verifications, planning, text: String::new(), presented: None }
    }

    pub(crate) fn observe(&mut self, event: &EngineEvent) {
        match event {
            EngineEvent::TextDelta(delta) => self.text.push_str(delta),
            EngineEvent::Asked(ask) => {
                if let AskKind::Plan { plan } = &ask.kind {
                    self.presented = Some(plan.clone());
                }
            }
            EngineEvent::ModeChanged(PermissionMode::Plan) => self.planning = true,
            EngineEvent::Completed { .. } => {
                let plan = match &self.presented {
                    Some(plan) => plan,
                    None if self.planning => &self.text,
                    None => return,
                };
                self.verifications.lock().unwrap_or_else(PoisonError::into_inner).plan_finished(plan);
            }
            _ => {}
        }
    }
}
