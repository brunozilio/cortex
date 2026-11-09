//! Run-local handles for background agents; execution lives in the agent loop.
use super::{Access, Outcome, Tool, Workspace};
use crate::model::ToolSpec;
use serde_json::{Value, json};
#[derive(Debug)]
struct TaskTool(&'static str);
impl Tool for TaskTool {
    fn spec(&self) -> ToolSpec {
        let (description, schema) = match self.0 {
            "TaskOutput" => (
                "Collect a background agent's report. block defaults to true; false polls without waiting.",
                json!({"type":"object","properties":{"task_id":{"type":"string"},"block":{"type":"boolean"}},"required":["task_id"],"additionalProperties":false}),
            ),
            "TaskStop" => (
                "Cancel a background agent and wait for its tools to stop.",
                json!({"type":"object","properties":{"task_id":{"type":"string"}},"required":["task_id"],"additionalProperties":false}),
            ),
            _ => (
                "List the background agents started in this run and their current status.",
                json!({"type":"object","properties":{},"additionalProperties":false}),
            ),
        };
        ToolSpec { name: self.0.into(), description: description.into(), schema }
    }
    fn access(&self) -> Access {
        Access::Reads
    }
    fn summary(&self, input: &Value) -> String {
        input["task_id"].as_str().unwrap_or_default().into()
    }
    fn run(&self, _: &Value, _: &Workspace) -> Outcome {
        Outcome::error("Task handles require the agent loop.")
    }
}
pub fn tools() -> Vec<Box<dyn Tool>> {
    let board = std::sync::Arc::new(std::sync::Mutex::new(Board::default()));
    let mut tools: Vec<Box<dyn Tool>> = vec![Box::new(TaskTool("TaskOutput")), Box::new(TaskTool("TaskStop"))];
    for name in ["TaskCreate", "TaskUpdate", "TaskGet", "TaskList"] {
        tools.push(Box::new(PlanTool { name, board: board.clone() }));
    }
    tools
}

#[derive(Debug, Default)]
struct Board {
    next: usize,
    tasks: std::collections::BTreeMap<String, Value>,
}
#[derive(Debug)]
struct PlanTool {
    name: &'static str,
    board: std::sync::Arc<std::sync::Mutex<Board>>,
}
impl Tool for PlanTool {
    fn spec(&self) -> ToolSpec {
        let (description, properties, required) = match self.name {
            "TaskCreate" => (
                "Create a planning task with a subject and description. Returns a run-local task_id.",
                json!({"subject":{"type":"string"},"description":{"type":"string"}}),
                json!(["subject"]),
            ),
            "TaskUpdate" => (
                "Update a planning task. Status is pending, in_progress or completed. blocked_by contains task_ids that must be completed first.",
                json!({"task_id":{"type":"string"},"status":{"enum":["pending","in_progress","completed"]},"description":{"type":"string"},"blocked_by":{"type":"array","items":{"type":"string"}}}),
                json!(["task_id"]),
            ),
            "TaskGet" => (
                "Read a planning task's description, status and dependencies.",
                json!({"task_id":{"type":"string"}}),
                json!(["task_id"]),
            ),
            _ => ("List planning tasks and background agents for this run.", json!({}), json!([])),
        };
        ToolSpec {
            name: self.name.into(),
            description: description.into(),
            schema: json!({"type":"object","properties":properties,"required":required,"additionalProperties":false}),
        }
    }
    fn access(&self) -> Access {
        Access::Reads
    }
    fn summary(&self, input: &Value) -> String {
        input["subject"].as_str().or_else(|| input["task_id"].as_str()).unwrap_or_default().into()
    }
    fn run(&self, input: &Value, _: &Workspace) -> Outcome {
        let mut board = self.board.lock().expect("task board");
        if self.name == "TaskList" {
            return Outcome::ok(json!(board.tasks.values().collect::<Vec<_>>()).to_string());
        }
        if self.name == "TaskCreate" {
            let Some(subject) = input["subject"].as_str().filter(|s| !s.trim().is_empty()) else {
                return Outcome::error("subject is required");
            };
            if board.tasks.len() >= 100 {
                return Outcome::error("Planning task limit reached (100).");
            }
            board.next += 1;
            let id = format!("plan-{}", board.next);
            let task = json!({"task_id":id,"subject":subject,"description":input["description"].as_str().unwrap_or_default(),"status":"pending","blocked_by":[]});
            board.tasks.insert(id, task.clone());
            return Outcome::ok(task.to_string());
        }
        let id = input["task_id"].as_str().unwrap_or_default();
        let Some(task) = board.tasks.get(id) else {
            return Outcome::error("Unknown planning task_id.");
        };
        if self.name == "TaskGet" {
            return Outcome::ok(task.to_string());
        }
        let mut updated = task.clone();
        if let Some(status) = input.get("status") {
            if !matches!(status.as_str(), Some("pending" | "in_progress" | "completed")) {
                return Outcome::error("Invalid planning status.");
            }
            updated["status"] = status.clone();
        }
        if let Some(description) = input.get("description") {
            if !description.is_string() {
                return Outcome::error("description must be text");
            }
            updated["description"] = description.clone();
        }
        if let Some(deps) = input.get("blocked_by") {
            let Some(deps) = deps.as_array() else {
                return Outcome::error("blocked_by must be an array");
            };
            for dep in deps {
                let Some(dep) = dep.as_str().filter(|dep| *dep != id && board.tasks.contains_key(*dep)) else {
                    return Outcome::error("Unknown or self-referencing dependency.");
                };
                let mut queue = vec![dep];
                let mut seen = std::collections::HashSet::new();
                while let Some(next) = queue.pop() {
                    if next == id {
                        return Outcome::error("Planning dependency cycle.");
                    }
                    if seen.insert(next) {
                        queue.extend(
                            board.tasks[next]["blocked_by"].as_array().into_iter().flatten().filter_map(Value::as_str),
                        );
                    }
                }
            }
            updated["blocked_by"] = json!(deps);
        }
        if updated["status"] != "pending"
            && updated["blocked_by"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .any(|dep| board.tasks[dep]["status"] != "completed")
        {
            return Outcome::error("Complete blocking tasks before starting or completing this task.");
        }
        board.tasks.insert(id.to_owned(), updated.clone());
        Outcome::ok(updated.to_string())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn planning_tasks_track_dependencies_and_reject_cycles_atomically() {
        let tools = tools();
        let workspace = Workspace::new(".");
        let run = |name: &str, input: Value| {
            tools.iter().find(|t| t.spec().name == name).expect("valid test fixture").run(&input, &workspace)
        };
        assert!(!run("TaskCreate", json!({"subject":"Build"})).is_error);
        assert!(!run("TaskCreate", json!({"subject":"Verify"})).is_error);
        assert!(!run("TaskUpdate", json!({"task_id":"plan-2","blocked_by":["plan-1"]})).is_error);
        assert!(run("TaskUpdate", json!({"task_id":"plan-1","blocked_by":["plan-2"]})).is_error);
        assert!(run("TaskUpdate", json!({"task_id":"plan-2","status":"completed"})).is_error);
        assert!(!run("TaskUpdate", json!({"task_id":"plan-1","status":"completed"})).is_error);
        assert!(!run("TaskUpdate", json!({"task_id":"plan-2","status":"completed"})).is_error);
    }
}
