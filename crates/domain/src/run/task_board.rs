use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const MOST: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    #[default]
    Pending,
    InProgress,
    Completed,
}

impl Status {
    pub fn named(name: &str) -> Option<Self> {
        match name {
            "pending" => Some(Self::Pending),
            "in_progress" => Some(Self::InProgress),
            "completed" => Some(Self::Completed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    pub task_id: String,
    pub subject: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub status: Status,
    #[serde(default)]
    pub blocked_by: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(default, rename = "activeForm", skip_serializing_if = "Option::is_none")]
    pub active_form: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Change {
    pub status: Option<Status>,
    pub description: Option<String>,
    pub blocked_by: Option<Vec<String>>,
    pub owner: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assigned {
    pub owner: String,
    pub task_id: String,
    pub subject: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Board {
    #[serde(default)]
    next: usize,
    #[serde(default)]
    tasks: BTreeMap<usize, Task>,
}

fn number(id: &str) -> Option<usize> {
    id.trim().trim_start_matches("plan-").parse().ok()
}

impl Board {
    pub fn create(&mut self, subject: &str, description: &str, active_form: Option<&str>) -> Result<Task, String> {
        let subject = subject.trim();
        if subject.is_empty() {
            return Err("subject is required".into());
        }
        if self.tasks.len() >= MOST {
            return Err(format!("The task list is full ({MOST})."));
        }
        self.next += 1;
        let task = Task {
            task_id: self.next.to_string(),
            subject: subject.to_owned(),
            description: description.to_owned(),
            status: Status::Pending,
            blocked_by: Vec::new(),
            owner: None,
            active_form: active_form.map(str::to_owned),
        };
        self.tasks.insert(self.next, task.clone());
        Ok(task)
    }

    fn find(&self, id: &str) -> Result<usize, String> {
        number(id).filter(|at| self.tasks.contains_key(at)).ok_or_else(|| format!("No task {id}."))
    }

    pub fn get(&self, id: &str) -> Result<Value, String> {
        let at = self.find(id)?;
        Ok(self.shown(&self.tasks[&at]))
    }

    pub fn update(&mut self, id: &str, change: Change, by: &str) -> Result<(Value, Option<Assigned>), String> {
        let at = self.find(id)?;
        let mut task = self.tasks[&at].clone();
        if let Some(deps) = change.blocked_by {
            task.blocked_by = self.checked_blockers(at, deps)?;
        }
        if let Some(description) = change.description {
            task.description = description;
        }
        let mut assigned = None;
        if let Some(owner) = change.owner.map(|owner| owner.trim().to_owned()) {
            match &task.owner {
                Some(held)
                    if *held != owner && !owner.is_empty() && task.status != Status::Completed && by != "team-lead" =>
                {
                    return Err(format!("Task {id} is already claimed by {held}."));
                }
                _ if task.status == Status::Completed && !owner.is_empty() => {
                    return Err(format!("Task {id} is already completed."));
                }
                _ => {}
            }
            if !owner.is_empty() && owner != by {
                assigned = Some(Assigned {
                    owner: owner.clone(),
                    task_id: task.task_id.clone(),
                    subject: task.subject.clone(),
                });
            }
            task.owner = (!owner.is_empty()).then_some(owner);
        }
        if let Some(status) = change.status {
            task.status = status;
        }
        if task.status != Status::Pending && self.open_blockers(&task) {
            return Err(format!("Task {id} is blocked: complete the tasks it waits for first."));
        }
        self.tasks.insert(at, task.clone());
        Ok((self.shown(&task), assigned))
    }

    fn checked_blockers(&self, at: usize, deps: Vec<String>) -> Result<Vec<String>, String> {
        let mut kept = Vec::new();
        for dep in deps {
            let dep_at = self.find(&dep)?;
            if dep_at == at {
                return Err("A task cannot wait for itself.".into());
            }
            let mut queue = vec![dep_at];
            let mut seen = HashSet::new();
            while let Some(next) = queue.pop() {
                if next == at {
                    return Err("That would make the tasks wait for each other in a loop.".into());
                }
                if seen.insert(next) {
                    queue.extend(self.tasks[&next].blocked_by.iter().filter_map(|id| number(id)));
                }
            }
            kept.push(dep_at.to_string());
        }
        Ok(kept)
    }

    fn open_blockers(&self, task: &Task) -> bool {
        task.blocked_by
            .iter()
            .filter_map(|id| number(id).and_then(|at| self.tasks.get(&at)))
            .any(|blocker| blocker.status != Status::Completed)
    }

    pub fn release(&mut self, owner: &str) -> Vec<String> {
        let mut released = Vec::new();
        for task in self.tasks.values_mut() {
            if task.owner.as_deref() == Some(owner) && task.status != Status::Completed {
                task.owner = None;
                task.status = Status::Pending;
                released.push(task.task_id.clone());
            }
        }
        released
    }

    fn shown(&self, task: &Task) -> Value {
        let blocks: Vec<&str> = self
            .tasks
            .values()
            .filter(|other| other.blocked_by.contains(&task.task_id))
            .map(|other| other.task_id.as_str())
            .collect();
        let mut shown = json!(task);
        shown["blocks"] = json!(blocks);
        shown
    }

    pub fn listed(&self) -> Vec<Value> {
        self.tasks.values().map(|task| self.shown(task)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn change() -> Change {
        Change::default()
    }

    #[test]
    fn tasks_get_ids_in_order_and_list_who_owns_them_and_what_they_block() {
        let mut board = Board::default();
        let build = board.create("Build", "compile it", Some("Building")).expect("build");
        let verify = board.create("Verify", "", None).expect("verify");
        assert_eq!((build.task_id.as_str(), verify.task_id.as_str()), ("1", "2"));
        board.update("2", Change { blocked_by: Some(vec!["1".into()]), ..change() }, "lead").expect("block");
        let listed = board.listed();
        assert_eq!(listed[0]["blocks"], serde_json::json!(["2"]));
        assert_eq!(listed[1]["blocked_by"], serde_json::json!(["1"]));
        assert_eq!(listed[0]["activeForm"], "Building");
    }

    #[test]
    fn dependencies_cannot_loop_and_a_blocked_task_cannot_start() {
        let mut board = Board::default();
        board.create("A", "", None).expect("a");
        board.create("B", "", None).expect("b");
        board.update("2", Change { blocked_by: Some(vec!["1".into()]), ..change() }, "lead").expect("block");
        assert!(board.update("1", Change { blocked_by: Some(vec!["2".into()]), ..change() }, "lead").is_err());
        assert!(board.update("1", Change { blocked_by: Some(vec!["1".into()]), ..change() }, "lead").is_err());
        let started = board.update("2", Change { status: Some(Status::InProgress), ..change() }, "lead");
        assert!(started.is_err_and(|error| error.contains("blocked")));
        board.update("1", Change { status: Some(Status::Completed), ..change() }, "lead").expect("done");
        board.update("2", Change { status: Some(Status::InProgress), ..change() }, "lead").expect("now it can");
    }

    #[test]
    fn a_task_is_claimed_once_and_its_owner_is_told_when_someone_else_assigns_it() {
        let mut board = Board::default();
        board.create("Fix lint", "", None).expect("task");
        let (_, assigned) =
            board.update("1", Change { owner: Some("tester".into()), ..change() }, "team-lead").expect("assign");
        assert_eq!(
            assigned,
            Some(Assigned { owner: "tester".into(), task_id: "1".into(), subject: "Fix lint".into() })
        );
        let taken = board.update("1", Change { owner: Some("reviewer".into()), ..change() }, "reviewer");
        assert!(taken.is_err_and(|error| error.contains("already claimed by tester")));
        let (_, assigned) =
            board.update("1", Change { owner: Some("tester".into()), ..change() }, "tester").expect("own claim");
        assert_eq!(assigned, None, "claiming for yourself tells nobody");
    }

    #[test]
    fn a_teammate_that_leaves_hands_back_its_open_tasks() {
        let mut board = Board::default();
        board.create("Open", "", None).expect("open");
        board.create("Done", "", None).expect("done");
        for id in ["1", "2"] {
            board
                .update(
                    id,
                    Change { owner: Some("tester".into()), status: Some(Status::InProgress), ..change() },
                    "tester",
                )
                .expect("claim");
        }
        board.update("2", Change { status: Some(Status::Completed), ..change() }, "tester").expect("complete");

        assert_eq!(board.release("tester"), ["1"]);

        let listed = board.listed();
        assert_eq!(
            (listed[0]["owner"].clone(), listed[0]["status"].clone()),
            (serde_json::Value::Null, "pending".into())
        );
        assert_eq!(listed[1]["owner"], "tester", "a finished task keeps its owner");
    }

    #[test]
    fn a_board_holds_at_most_a_hundred_tasks() {
        let mut board = Board::default();
        for n in 0..MOST {
            board.create(&format!("t{n}"), "", None).expect("room");
        }
        assert!(board.create("one more", "", None).is_err());
    }
}
