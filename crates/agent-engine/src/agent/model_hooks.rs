use serde_json::{Value, json};

use super::Agent;
use crate::model::TurnRequest;

impl Agent<'_> {
    pub(super) fn hooked_model(&self, event: &str, request: &TurnRequest, extra: Value) -> Option<String> {
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
            Ok(decision) => decision.model.filter(|model| *model != request.model),
            Err(error) => {
                tracing::warn!(%error, event, "a model hook failed; the model stays as it was");
                None
            }
        }
    }
}
