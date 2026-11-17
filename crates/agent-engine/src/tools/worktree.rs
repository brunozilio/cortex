use serde_json::Value;

use super::{Effect, Outcome, Tool, Workspace, field};
use crate::model::ToolSpec;

pub const ENTER_WORKTREE: &str = super::names::ENTER_WORKTREE;
pub const EXIT_WORKTREE: &str = super::names::EXIT_WORKTREE;

#[derive(Debug, Clone, Copy, Default)]
pub struct EnterWorktree;

impl Tool for EnterWorktree {
    fn spec(&self) -> ToolSpec {
        crate::tools::declared(ENTER_WORKTREE, &Value::Null)
    }

    fn effects(&self, _input: &Value, _workspace: &Workspace) -> Vec<Effect> {
        vec![Effect::Declared(super::Declared::Executes)]
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
        crate::tools::declared(EXIT_WORKTREE, &Value::Null)
    }

    fn effects(&self, _input: &Value, _workspace: &Workspace) -> Vec<Effect> {
        vec![Effect::Declared(super::Declared::Executes)]
    }

    fn summary(&self, input: &Value) -> String {
        field(input, "action").unwrap_or_default().to_owned()
    }

    fn run(&self, _input: &Value, _workspace: &Workspace) -> Outcome {
        Outcome::error("ExitWorktree is run by the agent loop.")
    }
}
