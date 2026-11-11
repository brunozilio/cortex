use serde_json::{Value, json};

use super::{Access, Outcome, Tool, Workspace};
use crate::model::ToolSpec;

pub const SEND_MESSAGE_TOOL: &str = "SendMessage";

#[derive(Debug, Clone, Copy, Default)]
pub struct SendMessageTool;

impl Tool for SendMessageTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: SEND_MESSAGE_TOOL.to_owned(),
            description: crate::tools::describe("SendMessage", &serde_json::Value::Null),
            schema: json!({
                "type": "object",
                "properties": {
                    "to": { "type": "string", "description": "The agent_id an Agent report gave (agent-1), or another session's name (ListSessions)." },
                    "message": { "type": "string", "description": "What to tell or ask the agent." },
                },
                "required": ["to", "message"],
            }),
        }
    }

    fn access(&self) -> Access {
        Access::Reads
    }

    fn summary(&self, input: &Value) -> String {
        input["to"].as_str().unwrap_or_default().to_owned()
    }

    fn run(&self, _input: &Value, _workspace: &Workspace) -> Outcome {
        Outcome::error("Messages to sub-agents can only be sent by the agent loop.")
    }
}
