use serde_json::{Value, json};

use super::{Access, Outcome, Tool, Workspace, field};
use crate::model::ToolSpec;

pub const ENTER_WORKTREE: &str = "EnterWorktree";
pub const EXIT_WORKTREE: &str = "ExitWorktree";

#[derive(Debug, Clone, Copy, Default)]
pub struct EnterWorktree;

impl Tool for EnterWorktree {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: ENTER_WORKTREE.into(),
            description: crate::tools::describe("EnterWorktree", &serde_json::Value::Null),
            schema: json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        }
    }

    fn access(&self) -> Access {
        Access::Executes
    }

    fn summary(&self, _input: &Value) -> String {
        String::new()
    }

    fn run(&self, _input: &Value, _workspace: &Workspace) -> Outcome {
        Outcome::error("EnterWorktree is run by the agent loop.")
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ExitWorktree;

impl Tool for ExitWorktree {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: EXIT_WORKTREE.into(),
            description: crate::tools::describe("ExitWorktree", &serde_json::Value::Null),
            schema: json!({
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["keep", "remove"] },
                    "discard_changes": { "type": "boolean" }
                },
                "required": ["action"],
                "additionalProperties": false
            }),
        }
    }

    fn access(&self) -> Access {
        Access::Executes
    }

    fn summary(&self, input: &Value) -> String {
        field(input, "action").unwrap_or_default().to_owned()
    }

    fn run(&self, _input: &Value, _workspace: &Workspace) -> Outcome {
        Outcome::error("ExitWorktree is run by the agent loop.")
    }
}
