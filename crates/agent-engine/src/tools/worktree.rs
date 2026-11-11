//! `EnterWorktree` and `ExitWorktree`: the main agent moves the session
//! into a worktree of its own, to work apart from the checkout, and back.
//! The agent loop runs them (the move happens between turns); this is
//! their schema and wording.

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
            description: concat!(
                "Move this session into a new git worktree of the project (a separate checkout of its \
                          base branch under ",
                brand::dir!(),
                "/worktrees), to work on a change apart from the user's \
                          checkout. From the next step on, files and commands are in the worktree, and later \
                          prompts of the session too, until ExitWorktree. Use it only when the user asks for a \
                          worktree or for working apart."
            )
            .into(),
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
            description: "Leave the worktree EnterWorktree made and go back to the project's checkout. \
                          action \"keep\" leaves the worktree on disk (its path is reported); \"remove\" deletes \
                          it, which is refused while it has uncommitted files or commits of its own unless \
                          discard_changes is true."
                .into(),
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
