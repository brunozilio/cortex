use std::sync::Arc;
use std::time::Instant;

use domain::{EngineEvent, EngineProblem, FailureKind, SessionCommand};

use super::MAX_OUTPUT;
use super::job::Job;
use crate::model::{ModelError, TurnRequest};
use crate::transcript::Message;

impl Job {
    pub(super) fn chore(&self, command: SessionCommand, history: &[Message], started: Instant) {
        let api = match self.source.api(&self.request) {
            Ok(api) => api,
            Err(error) => return self.model_failed(error),
        };
        let ledger = Arc::new(crate::meter::Ledger::default());
        let api = crate::meter::Metered {
            api,
            ledger: ledger.clone(),
            provider: self.request.model.provider,
            gate: self.gate.clone(),
            model: self.request.model.clone(),
        };
        let template = TurnRequest {
            model: self.request.model.id.clone(),
            system: crate::chores::AGENT.into(),
            messages: Vec::new(),
            tools: Vec::new(),
            effort: domain::Effort::Low,
            max_tokens: MAX_OUTPUT,
        };
        let stop = self.control.stop_flag();
        match crate::chores::answer(command, &api, &template, &self.request.prompt, history, stop) {
            Ok(Some(answer)) => {
                self.gate.emit(EngineEvent::Chore(answer));
            }
            Ok(None) => {}
            Err(error) => return self.model_failed(error),
        }
        self.completed(started, ledger.cost());
    }

    pub(super) fn completed(&self, started: Instant, cost_usd: Option<f64>) {
        if self.control.is_stopped() {
            self.gate.emit(EngineEvent::Cancelled);
            return;
        }
        let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        self.gate.emit(EngineEvent::Completed { duration_ms, cost_usd });
    }

    pub(super) fn model_failed(&self, error: ModelError) {
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
        self.fail(&error.to_string(), kind);
    }

    pub(super) fn fail(&self, message: &str, kind: FailureKind) {
        if self.control.is_stopped() {
            self.gate.emit(EngineEvent::Cancelled);
            return;
        }
        self.gate.emit(EngineEvent::Failure(kind));
        self.gate.emit(EngineEvent::Failed(message.to_owned()));
    }
}
