use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use platform::sync::Locked;

use super::Agent;
use super::conversations::Conversations;
use crate::control::Control;
use crate::tools::Outcome;
use crate::tools::builtin_agents;
use crate::transcript::Message;

#[derive(Default)]
pub(super) struct Tasks {
    pub(super) background: Mutex<BTreeMap<String, Arc<Task>>>,
    pub(super) conversations: Conversations,
    pub(super) repeats: Mutex<domain::repeats::Repeats>,
    pub(super) watches: Mutex<Vec<String>>,
    pub(super) stuck: Mutex<domain::stuck::Stuck>,
    pub(super) help_due: AtomicBool,
}

impl Tasks {
    pub(super) fn admit(&self, name: &str, input: &Value, may_change: bool) -> bool {
        self.repeats.locked().admit(name, &input.to_string(), may_change)
    }

    pub(super) fn watched(&self, id: String) {
        self.watches.locked().push(id);
    }

    pub(super) fn still_watching(&self, client: u64) -> Vec<String> {
        let active = crate::tools::watch::active(client);
        let started = self.watches.locked().clone();
        started.into_iter().filter(|id| active.iter().any(|watch| watch.id == *id)).collect()
    }
}

pub(super) struct StopTasksOnDrop<'a>(pub(super) &'a Tasks);

impl Drop for StopTasksOnDrop<'_> {
    fn drop(&mut self) {
        for task in self.0.background.locked().values() {
            task.control.cancel();
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
        let mut result = self.result.locked();
        while block && result.is_none() {
            result = platform::sync::wait(&self.ready, result);
        }
        if result.is_some() {
            self.collected.store(true, Ordering::Relaxed);
        }
        result.clone().unwrap_or_else(|| Outcome::ok("Task is still running."))
    }
}

impl Tasks {
    pub(super) fn busy(&self) -> bool {
        self.background.locked().values().any(|task| !task.collected.load(Ordering::Relaxed))
    }

    fn find(&self, input: &Value) -> Option<Arc<Task>> {
        self.background.locked().get(input["task_id"].as_str().unwrap_or_default()).cloned()
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
        let tasks = self.background.locked();
        let planning = planning();
        let background: Vec<Value> = tasks
            .iter()
            .map(|(id, task)| {
                let status = if task.result.locked().is_some() { "completed" } else { "running" };
                json!({"task_id": id, "status": status})
            })
            .collect();
        Outcome::ok(json!({"planning": planning, "background": background}).to_string())
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
        let conversations = tasks.conversations.clone();
        let mut tasks = tasks.background.locked();
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
                self.sub_agent_with(&input, &control, &workspace, &parent, &conversations, None)
            }))
            .unwrap_or_else(|_| Outcome::error("Background agent panicked."));
            *task.result.locked() = Some(result);
            task.ready.notify_all();
        }));
        Outcome::ok(json!({"task_id":id,"status":"running","instructions":"Use TaskOutput to collect the report before completing the request."}).to_string())
    }
}
