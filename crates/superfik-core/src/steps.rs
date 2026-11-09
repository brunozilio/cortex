//! The steps of a reply ("Read src/main.rs", "Bash cargo test"): what the
//! "Worked for 3s · 4 steps" line expands to.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::conversation::TurnResponse;
use crate::events::ToolCall;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StepKind {
    Read,
    Edit,
    Run,
    Search,
    Web,
    Agent,
    Skill,
    Reply,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Step {
    pub kind: StepKind,
    pub verb: String,
    pub target: String,
    #[serde(default)]
    pub failed: bool,
    #[serde(default)]
    pub output: Option<String>,
}

impl Step {
    pub fn of(call: &ToolCall, root: Option<&Path>) -> Self {
        let (kind, verb) = classify(&call.name);
        let target = match kind {
            StepKind::Read | StepKind::Edit => relative(&call.summary, root),
            _ => call.summary.clone(),
        };
        Self { kind, verb, target, failed: call.failed, output: call.output.clone() }
    }

    fn reply() -> Self {
        Self { kind: StepKind::Reply, verb: "Replied".into(), target: String::new(), failed: false, output: None }
    }
}

fn classify(name: &str) -> (StepKind, String) {
    let fixed = |kind: StepKind, verb: &str| (kind, verb.to_owned());
    match name {
        "Read" | "NotebookRead" => fixed(StepKind::Read, "Read"),
        "Glob" | "Grep" | "LS" => (StepKind::Search, name.to_owned()),
        "Edit" | "MultiEdit" | "NotebookEdit" => fixed(StepKind::Edit, "Edit"),
        "Write" => fixed(StepKind::Edit, "Write"),
        "Bash" | "BashOutput" => fixed(StepKind::Run, "Bash"),
        "WebSearch" => fixed(StepKind::Web, "Search"),
        "WebFetch" => fixed(StepKind::Web, "Fetch"),
        "Task" | "Agent" => fixed(StepKind::Agent, "Agent"),
        "Skill" => fixed(StepKind::Skill, "Skill"),
        "TodoWrite" => fixed(StepKind::Other, "Plan"),
        other => match other.strip_prefix("mcp__").and_then(|rest| rest.rsplit("__").next()) {
            Some(tool) => (StepKind::Other, tool.to_owned()),
            None => (StepKind::Other, other.to_owned()),
        },
    }
}

fn relative(path: &str, root: Option<&Path>) -> String {
    let Some(root) = root.map(|root| root.to_string_lossy().trim_end_matches('/').to_owned()) else {
        return path.to_owned();
    };
    match path.strip_prefix(root.as_str()) {
        Some(rest) if rest.starts_with('/') => rest.trim_start_matches('/').to_owned(),
        _ => path.to_owned(),
    }
}

impl TurnResponse {
    pub fn step_list(&self, root: Option<&Path>) -> Vec<Step> {
        let mut steps: Vec<Step> = self.tools.iter().map(|call| Step::of(call, root)).collect();
        if !self.text.is_empty() {
            steps.push(Step::reply());
        }
        steps
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(name: &str, summary: &str) -> Step {
        Step::of(&ToolCall::new("1", name, summary), Some(Path::new("/repo/")))
    }

    #[test]
    fn tools_get_short_verbs_and_relative_paths() {
        assert_eq!(step("Read", "/repo/src/main.rs").target, "src/main.rs");
        assert_eq!(step("Read", "/repository/x.rs").target, "/repository/x.rs");
        assert_eq!(step("Bash", "cargo test").verb, "Bash");
        assert_eq!(step("MultiEdit", "/repo/a.rs").verb, "Edit");
        assert_eq!(step("WebSearch", "night skies").kind, StepKind::Web);
        assert_eq!(step("Agent", "Explore the repo").verb, "Agent");
        assert_eq!(step("Grep", "fn main").verb, "Grep");
        assert_eq!(step("mcp__github__create_issue", "x").verb, "create_issue");
    }

    #[test]
    fn a_step_carries_its_error_and_output() {
        let mut call = ToolCall::new("1", "Bash", "cargo test");
        call.output = Some("2 errors".into());
        call.failed = true;
        let made = Step::of(&call, None);
        assert!(made.failed);
        assert_eq!(made.output.as_deref(), Some("2 errors"));
        assert!(!self::step("Read", "a.rs").failed);
    }

    #[test]
    fn step_list_matches_step_count() {
        let mut response = TurnResponse::default();
        response.tools.push(ToolCall::new("1", "Read", "a.rs"));
        response.text = "Done".into();
        let steps = response.step_list(None);
        assert_eq!(steps.len(), response.steps());
        assert_eq!(steps[1].kind, StepKind::Reply);
    }
}
