use domain::fallback::{Fallback, Verdict};
use serde_json::{Value, json};

use super::Agent;
use crate::hooks::Decision;
use crate::model::{ModelError, TurnRequest};

impl Agent<'_> {
    pub(super) fn next_model(
        &self,
        fallback: &mut Fallback,
        request: &TurnRequest,
        error: &ModelError,
    ) -> Option<String> {
        let failure = error.failure()?;
        fallback.failed(&request.model);
        let extra = json!({
            "error": error.to_string(),
            "errorKind": failure.label(),
            "attempt": fallback.tried().len(),
            "tried": fallback.tried(),
            "fallbacks": fallback.chain(),
        });
        let verdict = match self.model_hook("ModelError", request, extra) {
            Some(Decision { blocked: Some(_), .. }) => Verdict::Stop,
            Some(Decision { model: Some(model), .. }) => Verdict::Forward(model),
            _ => Verdict::Pass,
        };
        fallback.next(&request.model, failure, verdict)
    }

    pub(super) fn model_hook(&self, event: &str, request: &TurnRequest, extra: Value) -> Option<Decision> {
        if !self.workspace.hooks.0.contains_key(event) {
            return None;
        }
        let kind = if self.depth > 0 { "sub-agent" } else { "main" };
        let mut input = json!({
            "model": request.model,
            "kind": kind,
            "mode": domain::command::mode_label(self.control.mode()),
            "messages": request.messages.len(),
        });
        if let (Some(input), Value::Object(extra)) = (input.as_object_mut(), extra) {
            input.extend(extra);
        }
        match self.workspace.hooks.run(event, kind, input, self.workspace) {
            Ok(decision) => Some(decision),
            Err(error) => {
                tracing::warn!(%error, event, "a model hook failed; the model stays as it was");
                None
            }
        }
    }
}
