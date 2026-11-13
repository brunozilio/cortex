use super::Agent;
use crate::model::{ToolSpec, TurnRequest};

impl Agent<'_> {
    pub(super) fn turn_template(&self, fell_back: Option<&String>) -> TurnRequest {
        TurnRequest {
            messages: Vec::new(),
            system: self.request.system.clone(),
            tools: self.offered(),
            model: fell_back.cloned().unwrap_or_else(|| self.request.model.clone()),
            ..self.request.clone()
        }
    }

    pub(super) fn offered(&self) -> Vec<ToolSpec> {
        let mut tools = self.request.tools.clone();
        if let Some(deferred) = &self.workspace.services.deferred {
            tools.extend(deferred.loaded());
        }
        tools
    }
}
