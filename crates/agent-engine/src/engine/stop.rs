use std::time::Instant;

use domain::{EngineEvent, EngineProblem, FailureKind};

use super::job::Job;
use crate::model::ModelError;

#[derive(Debug)]
pub(super) enum Stop {
    Completed { cost_usd: Option<f64> },
    Cancelled,
    Failed { kind: FailureKind, message: String },
    Diagnosed { problem: EngineProblem, message: String },
    Model(ModelError),
}

impl Stop {
    pub(super) fn failed(message: impl Into<String>) -> Self {
        Self::Failed { kind: FailureKind::Other, message: message.into() }
    }

    pub(super) fn completed(cost_usd: Option<f64>) -> Self {
        Self::Completed { cost_usd }
    }
}

impl Job {
    pub(super) fn end(&self, stop: Stop, started: Instant) {
        match stop {
            Stop::Completed { cost_usd } => self.completed(started, cost_usd),
            Stop::Cancelled => {
                self.gate.emit(EngineEvent::Cancelled);
            }
            Stop::Failed { kind, message } => self.failed(&message, kind),
            Stop::Diagnosed { problem, message } => {
                self.gate.emit(EngineEvent::Diagnosed(problem));
                self.failed(&message, FailureKind::Other);
            }
            Stop::Model(error) => self.model_failed(error),
        }
    }

    fn completed(&self, started: Instant, cost_usd: Option<f64>) {
        if self.control.is_stopped() {
            self.gate.emit(EngineEvent::Cancelled);
            return;
        }
        let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        self.gate.emit(EngineEvent::Completed { duration_ms, cost_usd });
    }

    fn model_failed(&self, error: ModelError) {
        if self.control.is_stopped() {
            self.gate.emit(EngineEvent::Cancelled);
            return;
        }
        let kind = match &error {
            ModelError::Unauthorized(_) => {
                self.gate.emit(EngineEvent::Diagnosed(EngineProblem::NotLoggedIn));
                FailureKind::Other
            }
            ModelError::Overloaded(_) => FailureKind::Unavailable,
            ModelError::Cancelled => {
                self.gate.emit(EngineEvent::Cancelled);
                return;
            }
            _ => FailureKind::Other,
        };
        self.failed(&error.to_string(), kind);
    }

    fn failed(&self, message: &str, kind: FailureKind) {
        if self.control.is_stopped() {
            self.gate.emit(EngineEvent::Cancelled);
            return;
        }
        self.gate.emit(EngineEvent::Failure(kind));
        self.gate.emit(EngineEvent::Failed(message.to_owned()));
    }
}
