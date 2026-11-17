use domain::EngineEvent;
use serde_json::{Value, json};

use crate::agent::Agent;
use crate::agent::run::ThisRun;
use crate::tools::names;

impl Agent<'_> {
    fn listed_tasks(&self, run: &ThisRun) -> Option<Vec<Value>> {
        let list = self.tool(run, names::TASK_LIST)?;
        match serde_json::from_str::<Value>(&list.run(&json!({}), self.workspace).content) {
            Ok(Value::Array(tasks)) => Some(tasks),
            _ => None,
        }
    }

    pub(in crate::agent) fn open_tasks(&self, run: &ThisRun) -> Vec<String> {
        self.listed_tasks(run).map(|tasks| still_open(&tasks)).unwrap_or_default()
    }

    pub(in crate::agent) fn emit_tasks(&self, run: &ThisRun) {
        let Some(tasks) = self.listed_tasks(run) else { return };
        (self.emit)(EngineEvent::Tasks(as_items(&tasks)));
    }
}

fn still_open(tasks: &[Value]) -> Vec<String> {
    tasks
        .iter()
        .filter(|task| task["status"].as_str() != Some("completed"))
        .map(|task| {
            let status = task["status"].as_str().unwrap_or("pending");
            format!("{} ({status})", task["subject"].as_str().unwrap_or_default())
        })
        .collect()
}

fn as_items(tasks: &[Value]) -> Vec<domain::events::TaskItem> {
    tasks
        .iter()
        .map(|task| domain::events::TaskItem {
            id: task["task_id"].as_str().unwrap_or_default().to_owned(),
            subject: task["subject"].as_str().unwrap_or_default().to_owned(),
            status: task["status"].as_str().unwrap_or("pending").to_owned(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_tasks_leave_out_the_completed_and_say_where_each_stands() {
        let tasks = [
            json!({"task_id": "plan-1", "subject": "Build", "status": "completed"}),
            json!({"task_id": "plan-2", "subject": "Verify", "status": "in_progress"}),
            json!({"task_id": "plan-3", "subject": "Ship"}),
        ];
        assert_eq!(still_open(&tasks), ["Verify (in_progress)", "Ship (pending)"]);
        let items = as_items(&tasks);
        assert_eq!(items.len(), 3);
        assert_eq!((items[2].id.as_str(), items[2].status.as_str()), ("plan-3", "pending"));
    }
}
