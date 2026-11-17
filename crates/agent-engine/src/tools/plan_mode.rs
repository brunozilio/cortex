use serde_json::Value;

use super::{Effect, Outcome, Tool, Workspace};
use crate::model::ToolSpec;

pub const EXIT_PLAN_TOOL: &str = super::names::EXIT_PLAN;
pub const ENTER_PLAN_TOOL: &str = super::names::ENTER_PLAN;

#[derive(Debug, Clone, Copy, Default)]
pub struct ExitPlanMode;

impl Tool for ExitPlanMode {
    fn spec(&self) -> ToolSpec {
        crate::tools::declared(EXIT_PLAN_TOOL, &Value::Null)
    }

    fn effects(&self, _input: &Value, _workspace: &Workspace) -> Vec<Effect> {
        Vec::new()
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
        crate::tools::declared(ENTER_PLAN_TOOL, &Value::Null)
    }

    fn effects(&self, _input: &Value, _workspace: &Workspace) -> Vec<Effect> {
        Vec::new()
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
    use serde_json::json;

    #[test]
    fn a_plan_must_have_text() {
        assert_eq!(plan(&json!({"plan": " ## Steps\n"})), Some("## Steps"));
        assert_eq!(plan(&json!({"plan": "  "})), None);
        assert_eq!(plan(&json!({})), None);
    }
}
