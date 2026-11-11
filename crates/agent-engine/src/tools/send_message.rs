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
            description: GUIDE.to_owned(),
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

const GUIDE: &str = "\
Continues a sub-agent you started earlier in this request, with its conversation intact: it remembers its task, \
what it read and what it reported. `to` is the agent_id at the end of its report. Use it for a follow-up, a \
correction or the next step of the same work, instead of starting a fresh agent and briefing it again. The \
agent answers with a new report, which, like the first, is not shown to the user.

- An agent still running in the background cannot take a message: collect it with TaskOutput first.
- Agents last for this request only; start a new one for later requests.
- `to` can also name another of the user's open sessions (ListSessions lists them): the message waits in its inbox \
and reaches its agent; any answer comes back to you as an event.";
