//! A run's plan, read as it comes so that the finished plan's
//! `Verification` section becomes the session's criteria: the one the
//! agent presented with `ExitPlanMode`, else a planning run's text.

use std::sync::{Arc, Mutex, PoisonError};

use domain::{AskKind, EngineEvent, PermissionMode, Verifications};

#[derive(Debug)]
pub(crate) struct PlanWatch {
    verifications: Arc<Mutex<Verifications>>,
    /// Whether the run plans: it started in plan mode or the agent entered it.
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
