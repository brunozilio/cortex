//! `ExitPlanMode` and `EnterPlanMode`: the agent puts its plan to the user,
//! or turns planning on for a task that needs one. The agent loop runs them
//! (leaving waits for the user); this is their schema and wording.

use serde_json::{Value, json};

use super::{Access, Outcome, Tool, Workspace};
use crate::model::ToolSpec;

pub const EXIT_PLAN_TOOL: &str = "ExitPlanMode";
pub const ENTER_PLAN_TOOL: &str = "EnterPlanMode";

#[derive(Debug, Clone, Copy, Default)]
pub struct ExitPlanMode;

impl Tool for ExitPlanMode {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: EXIT_PLAN_TOOL.to_owned(),
            description: "In planning mode, present your finished plan to the user for approval. The user reads it \
                          and either approves it, which ends planning so you start implementing it right away, or \
                          keeps planning and tells you what to change. Pass the whole plan, in markdown, as `plan`; \
                          do not also write it as a message. Use it only when the task needs code changes and the \
                          plan is complete; to clarify requirements or choose between approaches, ask with \
                          AskUserQuestion first. Never ask in text whether the plan is fine: this tool is that \
                          question."
                .to_owned(),
            schema: json!({
                "type": "object",
                "properties": {
                    "plan": { "type": "string", "description": "The plan, in markdown." },
                },
                "required": ["plan"],
                "additionalProperties": false,
            }),
        }
    }

    fn access(&self) -> Access {
        Access::Reads
    }

    fn summary(&self, _input: &Value) -> String {
        String::new()
    }

    fn run(&self, _input: &Value, _workspace: &Workspace) -> Outcome {
        Outcome::error("ExitPlanMode is run by the agent loop.")
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct EnterPlanMode;

impl Tool for EnterPlanMode {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: ENTER_PLAN_TOOL.to_owned(),
            description: "Turn on planning mode before a non-trivial change: a new feature, several valid \
                          approaches, changes across many files, or requirements you need to explore first. While \
                          planning you only read and search, then present the plan with ExitPlanMode for the \
                          user's approval. Skip it for small, obvious changes, for questions and research, and when \
                          the user has asked you to just do it."
                .to_owned(),
            schema: json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        }
    }

    fn access(&self) -> Access {
        Access::Reads
    }

    fn summary(&self, _input: &Value) -> String {
        String::new()
    }

    fn run(&self, _input: &Value, _workspace: &Workspace) -> Outcome {
        Outcome::error("EnterPlanMode is run by the agent loop.")
    }
}

/// The plan an `ExitPlanMode` call carries, if it has one.
pub fn plan(input: &Value) -> Option<&str> {
    input["plan"].as_str().map(str::trim).filter(|plan| !plan.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plan_must_have_text() {
        assert_eq!(plan(&json!({"plan": " ## Steps\n"})), Some("## Steps"));
        assert_eq!(plan(&json!({"plan": "  "})), None);
        assert_eq!(plan(&json!({})), None);
    }
}
