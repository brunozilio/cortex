//! Background agents of one run: started by the `Agent` tool, collected
//! with `TaskOutput`, stopped with `TaskStop`, listed with `TaskList`, and
//! all cancelled when the run ends.

use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use super::Agent;
use crate::control::Control;
use crate::tools::Outcome;
use crate::tools::builtin_agents;
use crate::transcript::Message;

#[derive(Default)]
pub(super) struct Tasks(pub(super) Mutex<BTreeMap<String, Arc<Task>>>);

pub(super) struct StopTasksOnDrop<'a>(pub(super) &'a Tasks);

impl Drop for StopTasksOnDrop<'_> {
    fn drop(&mut self) {
        if let Ok(tasks) = self.0.0.lock() {
            for task in tasks.values() {
                task.control.cancel();
            }
        }
    }
}

pub(super) struct Task {
    pub(super) result: Mutex<Option<Outcome>>,
    pub(super) ready: Condvar,
    pub(super) collected: AtomicBool,
    pub(super) control: Arc<Control>,
}

impl Task {
    pub(super) fn output(&self, block: bool) -> Outcome {
        let mut result = self.result.lock().expect("task result");
        while block && result.is_none() {
            result = self.ready.wait(result).expect("task result");
        }
        if result.is_some() {
            self.collected.store(true, Ordering::Relaxed);
        }
        result.clone().unwrap_or_else(|| Outcome::ok("Task is still running."))
    }
}

impl Tasks {
    fn find(&self, input: &Value) -> Option<Arc<Task>> {
        self.0.lock().expect("tasks").get(input["task_id"].as_str().unwrap_or_default()).cloned()
    }

    pub(super) fn output(&self, input: &Value) -> Outcome {
        match self.find(input) {
            Some(task) => task.output(input["block"].as_bool().unwrap_or(true)),
            None => Outcome::error("Unknown task_id for this run."),
        }
    }

    pub(super) fn stop(&self, input: &Value) -> Outcome {
        match self.find(input) {
            Some(task) => {
                task.control.cancel();
                task.output(true);
                Outcome::ok("Task stopped and joined.")
            }
            None => Outcome::error("Unknown task_id for this run."),
        }
    }

    pub(super) fn list(&self, planning: impl FnOnce() -> Value) -> Outcome {
        let tasks = self.0.lock().expect("tasks");
        let planning = planning();
        Outcome::ok(json!({"planning":planning,"background":tasks.iter().map(|(id,t)| json!({"task_id":id,"status":if t.result.lock().expect("task").is_some(){"completed"}else{"running"}})).collect::<Vec<_>>()}).to_string())
    }
}

impl Agent<'_> {
    #[allow(clippy::needless_lifetimes)]
    pub(super) fn start_background<'env>(
        &'env self,
        input: &Value,
        tasks: &Tasks,
        parent: &[Message],
        spawn: &dyn Fn(Box<dyn FnOnce() + Send + 'env>),
    ) -> Outcome {
        let mut tasks = tasks.0.lock().expect("tasks");
        if self.depth > 0 || tasks.len() >= 16 {
            return Outcome::error("Background agent limit reached (16 per run; no nested agents).");
        }
        let id = format!("task-{}", tasks.len() + 1);
        let control = self.control.child();
        let workspace = self.workspace.fork(control.shared_stop());
        let task = Arc::new(Task {
            result: Mutex::new(None),
            ready: Condvar::new(),
            collected: AtomicBool::new(false),
            control: control.clone(),
        });
        tasks.insert(id.clone(), task.clone());
        let input = input.clone();
        let parent = if builtin_agents::is_fork(&input) { parent.to_vec() } else { Vec::new() };
        spawn(Box::new(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.sub_agent_with(&input, &control, &workspace, &parent)
            }))
            .unwrap_or_else(|_| Outcome::error("Background agent panicked."));
            *task.result.lock().expect("task result") = Some(result);
            task.ready.notify_all();
        }));
        Outcome::ok(json!({"task_id":id,"status":"running","instructions":"Use TaskOutput to collect the report before completing the request."}).to_string())
    }
}
