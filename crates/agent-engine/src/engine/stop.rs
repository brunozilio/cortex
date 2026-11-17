use std::time::Instant;

use domain::failure::RunFailure;
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
        RunFailure::other(message).into()
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
            Stop::Failed { kind, message } => self.failed(RunFailure { kind, message }),
            Stop::Diagnosed { problem, message } => {
                self.gate.emit(EngineEvent::Diagnosed(problem));
                self.failed(RunFailure::other(message));
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
        match &error {
            ModelError::Unauthorized(_) => {
                self.gate.emit(EngineEvent::Diagnosed(EngineProblem::NotLoggedIn));
            }
            ModelError::Cancelled => {
                self.gate.emit(EngineEvent::Cancelled);
                return;
            }
            _ => {}
        }
        self.failed(RunFailure::from(error));
    }

    fn failed(&self, failure: RunFailure) {
        if self.control.is_stopped() {
            self.gate.emit(EngineEvent::Cancelled);
            return;
        }
        for event in failure.events() {
            self.gate.emit(event);
        }
    }
}

impl From<RunFailure> for Stop {
    fn from(failure: RunFailure) -> Self {
        Self::Failed { kind: failure.kind, message: failure.message }
    }
}

impl From<ModelError> for Stop {
    fn from(error: ModelError) -> Self {
        Self::Model(error)
    }
}
