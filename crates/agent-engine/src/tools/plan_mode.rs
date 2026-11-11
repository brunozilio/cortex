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
            description: crate::tools::describe("ExitPlanMode", &serde_json::Value::Null),
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
            description: crate::tools::describe("EnterPlanMode", &serde_json::Value::Null),
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
