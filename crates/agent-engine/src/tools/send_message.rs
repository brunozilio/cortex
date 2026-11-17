use serde_json::Value;

use super::{Effect, Outcome, Tool, Workspace};
use crate::model::ToolSpec;

pub const SEND_MESSAGE_TOOL: &str = super::names::SEND_MESSAGE;

#[derive(Debug, Clone, Copy, Default)]
pub struct SendMessageTool;

impl Tool for SendMessageTool {
    fn spec(&self) -> ToolSpec {
        crate::tools::declared(SEND_MESSAGE_TOOL, &Value::Null)
    }

    fn effects(&self, _input: &Value, _workspace: &Workspace) -> Vec<Effect> {
        vec![Effect::Spawn(super::SpawnKind::Agent)]
    }

    fn summary(&self, input: &Value) -> String {
        input["to"].as_str().unwrap_or_default().to_owned()
    }

    fn run(&self, _input: &Value, _workspace: &Workspace) -> Outcome {
        Outcome::error("Messages to sub-agents can only be sent by the agent loop.")
    }
}
