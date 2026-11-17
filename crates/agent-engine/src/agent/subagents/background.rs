use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use platform::sync::Locked;

use crate::agent::Agent;
use crate::agent::run::ThisRun;
use crate::control::Control;
use crate::tools::Outcome;
use crate::tools::builtin_agents;
use crate::transcript::Message;

const BACKGROUND_AT_MOST: usize = 16;

pub(in crate::agent) type Spawn<'s, 'env> = &'s dyn Fn(Box<dyn FnOnce() + Send + 'env>);

pub(in crate::agent) type BackgroundJob<'env> =
    Box<dyn FnOnce(&Arc<Control>, &crate::tools::Workspace) -> Outcome + Send + 'env>;

#[derive(Default)]
pub(in crate::agent) struct Background(Mutex<BTreeMap<String, Arc<BackgroundAgent>>>);

pub(in crate::agent) struct CancelOnDrop<'a>(pub(in crate::agent) &'a Background);

impl Drop for CancelOnDrop<'_> {
    fn drop(&mut self) {
        self.0.cancel_all();
    }
}

pub(in crate::agent) struct BackgroundAgent {
    result: Mutex<Option<Outcome>>,
    ready: Condvar,
    collected: AtomicBool,
    control: Arc<Control>,
}

impl BackgroundAgent {
    fn started(control: Arc<Control>) -> Self {
        Self { result: Mutex::new(None), ready: Condvar::new(), collected: AtomicBool::new(false), control }
    }

    pub(in crate::agent) fn output(&self, block: bool) -> Outcome {
        let mut result = self.result.locked();
        while block && result.is_none() {
            result = platform::sync::wait(&self.ready, result);
        }
        if result.is_some() {
            self.collected.store(true, Ordering::Relaxed);
        }
        result.clone().unwrap_or_else(|| Outcome::ok("Task is still running."))
    }

    pub(in crate::agent) fn is_collected(&self) -> bool {
        self.collected.load(Ordering::Relaxed)
    }

    fn finished(&self, result: Outcome) {
        *self.result.locked() = Some(result);
        self.ready.notify_all();
    }
}

impl Background {
    pub(in crate::agent) fn cancel_all(&self) {
        for task in self.0.locked().values() {
            task.control.cancel();
        }
    }

    pub(in crate::agent) fn busy(&self) -> bool {
        self.0.locked().values().any(|task| !task.is_collected())
    }

    pub(in crate::agent) fn started(&self) -> Vec<(String, Arc<BackgroundAgent>)> {
        self.0.locked().iter().map(|(id, task)| (id.clone(), task.clone())).collect()
    }

    fn has_room(&self) -> bool {
        self.0.locked().len() < BACKGROUND_AT_MOST
    }

    fn start(&self, control: Arc<Control>) -> Option<(String, Arc<BackgroundAgent>)> {
        let mut tasks = self.0.locked();
        if tasks.len() >= BACKGROUND_AT_MOST {
            return None;
        }
        let id = format!("task-{}", tasks.len() + 1);
        let task = Arc::new(BackgroundAgent::started(control));
        tasks.insert(id.clone(), task.clone());
        Some((id, task))
    }

    fn find(&self, input: &Value) -> Option<Arc<BackgroundAgent>> {
        self.0.locked().get(input["task_id"].as_str().unwrap_or_default()).cloned()
    }

    pub(in crate::agent) fn output(&self, input: &Value) -> Outcome {
        match self.find(input) {
            Some(task) => task.output(input["block"].as_bool().unwrap_or(true)),
            None => Outcome::error("Unknown task_id for this run."),
        }
    }

    pub(in crate::agent) fn stop(&self, input: &Value) -> Outcome {
        match self.find(input) {
            Some(task) => {
                task.control.cancel();
                task.output(true);
                Outcome::ok("Task stopped and joined.")
            }
            None => Outcome::error("Unknown task_id for this run."),
        }
    }

    pub(in crate::agent) fn list(&self, planning: impl FnOnce() -> Value) -> Outcome {
        let tasks = self.0.locked();
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

fn limit_reached(what: &str) -> Outcome {
    Outcome::error(format!("Background {what} limit reached (16 per run; no nested agents)."))
}

impl Agent<'_> {
    pub(in crate::agent) fn can_background(&self, run: &ThisRun) -> bool {
        self.depth == 0 && run.background.has_room()
    }

    pub(in crate::agent) fn start_background<'env>(
        &'env self,
        input: &Value,
        run: &ThisRun,
        parent: &[Message],
        spawn: Spawn<'_, 'env>,
    ) -> Outcome {
        let conversations = run.conversations.clone();
        let input = input.clone();
        let parent = if builtin_agents::is_fork(&input) { parent.to_vec() } else { Vec::new() };
        let job = move |control: &Arc<Control>, workspace: &crate::tools::Workspace| {
            self.sub_agent_with(&input, control, workspace, &parent, &conversations, None)
        };
        match self.in_background(run, "agent", Box::new(job), spawn) {
            Ok(id) => Outcome::ok(json!({"task_id":id,"status":"running","instructions":"Use TaskOutput to collect the report before completing the request."}).to_string()),
            Err(refused) => refused,
        }
    }

    pub(in crate::agent) fn in_background<'env>(
        &'env self,
        run: &ThisRun,
        what: &str,
        job: BackgroundJob<'env>,
        spawn: Spawn<'_, 'env>,
    ) -> Result<String, Outcome> {
        if !self.can_background(run) {
            return Err(limit_reached(what));
        }
        let control = self.control.child();
        let Some((id, task)) = run.background.start(control.clone()) else { return Err(limit_reached(what)) };
        let workspace = self.workspace.fork(control.shared_stop());
        let panicked = format!("Background {what} panicked.");
        spawn(Box::new(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| job(&control, &workspace)))
                .unwrap_or_else(|_| Outcome::error(panicked));
            task.finished(result);
        }));
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn control() -> Arc<Control> {
        Arc::new(Control::new(domain::PermissionMode::Ask, Arc::default()))
    }

    #[test]
    fn a_started_task_is_listed_running_then_collected_once_it_reports() {
        let background = Background::default();
        let (id, task) = background.start(control()).expect("room");
        assert_eq!(id, "task-1");
        assert!(background.busy());
        assert_eq!(background.output(&json!({"task_id": id, "block": false})).content, "Task is still running.");
        assert!(background.busy(), "asking early collects nothing");
        task.finished(Outcome::ok("report"));
        assert!(background.started().iter().all(|(_, task)| !task.is_collected()));
        assert_eq!(background.output(&json!({"task_id": id})).content, "report");
        assert!(!background.busy());
        assert!(background.started().iter().all(|(_, task)| task.is_collected()));
        assert!(background.output(&json!({"task_id": "task-9"})).is_error);
    }

    #[test]
    fn no_more_than_sixteen_tasks_start_in_one_run() {
        let background = Background::default();
        for _ in 0..BACKGROUND_AT_MOST {
            assert!(background.start(control()).is_some());
        }
        assert!(!background.has_room());
        assert!(background.start(control()).is_none());
    }
}
