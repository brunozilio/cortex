use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use domain::task_board::{Board, Change, Status};
use domain::team::Protocol;
use platform::store::Document;
use platform::sync::Locked;
use serde_json::{Value, json};

use super::names::ToolName;
use super::{Effect, Outcome, Tool, Workspace};
use crate::model::ToolSpec;

#[derive(Debug)]
struct TaskTool(ToolName);
impl Tool for TaskTool {
    fn spec(&self) -> ToolSpec {
        crate::tools::declared(self.0.as_str(), &Value::Null)
    }
    fn effects(&self, _: &Value, _: &Workspace) -> Vec<Effect> {
        let stops = self.0 == ToolName::TaskStop;
        stops.then_some(Effect::Cancel { own: true }).into_iter().collect()
    }
    fn summary(&self, input: &Value) -> String {
        input["task_id"].as_str().unwrap_or_default().into()
    }
    fn run(&self, _: &Value, _: &Workspace) -> Outcome {
        Outcome::error("Task handles require the agent loop.")
    }
}

pub fn tools() -> Vec<Box<dyn Tool>> {
    let board: Arc<Mutex<Board>> = Arc::default();
    let mut tools: Vec<Box<dyn Tool>> =
        vec![Box::new(TaskTool(ToolName::TaskOutput)), Box::new(TaskTool(ToolName::TaskStop))];
    for name in [ToolName::TaskCreate, ToolName::TaskUpdate, ToolName::TaskGet, ToolName::TaskList] {
        tools.push(Box::new(PlanTool { name, board: Arc::clone(&board) }));
    }
    tools
}

#[derive(Debug)]
struct PlanTool {
    name: ToolName,
    board: Arc<Mutex<Board>>,
}

fn shared(workspace: &Workspace) -> Option<PathBuf> {
    crate::team::task_list(workspace.home.as_deref(), &workspace.session_id, workspace.teammate.as_ref())
}

fn me(workspace: &Workspace) -> String {
    crate::team::current(workspace.home.as_deref(), &workspace.session_id, workspace.teammate.as_ref())
        .map_or_else(|| domain::team::LEAD.to_owned(), |me| me.name)
}

impl PlanTool {
    fn on_board<R>(
        &self,
        workspace: &Workspace,
        work: impl FnOnce(&mut Board) -> Result<R, String>,
    ) -> Result<R, String> {
        match shared(workspace) {
            Some(path) => Document::at(path).try_update(work).map_err(|error| error.to_string())?,
            None => work(&mut self.board.locked()),
        }
    }

    fn change(input: &Value) -> Result<Change, String> {
        let status = match input.get("status").and_then(Value::as_str) {
            Some(name) => Some(Status::named(name).ok_or("status is pending, in_progress or completed")?),
            None => None,
        };
        let blocked_by = match input.get("blocked_by").or_else(|| input.get("addBlockedBy")) {
            Some(deps) => Some(
                deps.as_array()
                    .ok_or("blocked_by must be an array of task ids")?
                    .iter()
                    .map(|dep| dep.as_str().map(str::to_owned).ok_or("blocked_by must be an array of task ids"))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            None => None,
        };
        let description = match input.get("description") {
            Some(text) => Some(text.as_str().ok_or("description must be text")?.to_owned()),
            None => None,
        };
        Ok(Change { status, description, blocked_by, owner: input["owner"].as_str().map(str::to_owned) })
    }

    fn told(workspace: &Workspace, assigned: domain::task_board::Assigned, by: &str) {
        let Some(me) =
            crate::team::current(workspace.home.as_deref(), &workspace.session_id, workspace.teammate.as_ref())
        else {
            return;
        };
        if !me.team.knows(&assigned.owner) {
            return;
        }
        let message = Protocol::TaskAssignment {
            task_id: assigned.task_id,
            subject: assigned.subject.clone(),
            assigned_by: by.to_owned(),
        };
        let _ = me.team.send(&assigned.owner, by, &message.text(), Some(&format!("task: {}", assigned.subject)));
    }

    fn hooked(workspace: &Workspace, event: crate::hooks::HookEvent, task: Value) -> Result<(), String> {
        let input = json!({ "task": task, "teammate_name": me(workspace) });
        let ran = workspace.hooks.run(event, "", input, workspace)?;
        match ran.blocked {
            Some(reason) => Err(format!("A {} hook refused it: {reason}", event.as_str())),
            None => Ok(()),
        }
    }

    fn answered(&self, input: &Value, workspace: &Workspace) -> Result<Value, String> {
        let completing = input["status"].as_str() == Some("completed");
        match self.name {
            ToolName::TaskCreate => {
                let proposed = json!({ "subject": input["subject"], "description": input["description"] });
                Self::hooked(workspace, crate::hooks::HookEvent::TaskCreated, proposed)?;
            }
            ToolName::TaskUpdate if completing => {
                let id = input["task_id"].as_str().unwrap_or_default();
                let task = self.on_board(workspace, |board| board.get(id))?;
                Self::hooked(workspace, crate::hooks::HookEvent::TaskCompleted, task)?;
            }
            _ => {}
        }
        match self.name {
            ToolName::TaskList => self.on_board(workspace, |board| Ok(json!(board.listed()))),
            ToolName::TaskCreate => self.on_board(workspace, |board| {
                let subject = input["subject"].as_str().unwrap_or_default();
                let description = input["description"].as_str().unwrap_or_default();
                board.create(subject, description, input["activeForm"].as_str()).map(|task| json!(task))
            }),
            ToolName::TaskGet => {
                let id = input["task_id"].as_str().unwrap_or_default();
                self.on_board(workspace, |board| board.get(id))
            }
            _ => {
                let id = input["task_id"].as_str().unwrap_or_default();
                let change = Self::change(input)?;
                let by = me(workspace);
                let (task, assigned) = self.on_board(workspace, |board| board.update(id, change, &by))?;
                if let Some(assigned) = assigned {
                    Self::told(workspace, assigned, &by);
                }
                Ok(task)
            }
        }
    }
}

impl Tool for PlanTool {
    fn spec(&self) -> ToolSpec {
        crate::tools::declared(self.name.as_str(), &Value::Null)
    }
    fn effects(&self, _: &Value, _: &Workspace) -> Vec<Effect> {
        Vec::new()
    }
    fn summary(&self, input: &Value) -> String {
        input["subject"].as_str().or_else(|| input["task_id"].as_str()).unwrap_or_default().into()
    }
    fn run(&self, input: &Value, workspace: &Workspace) -> Outcome {
        match self.answered(input, workspace) {
            Ok(value) => Outcome::ok(value.to_string()),
            Err(problem) => Outcome::error(problem),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runner(workspace: &Workspace) -> impl Fn(&str, Value) -> Outcome + '_ {
        let tools = tools();
        move |name: &str, input: Value| {
            tools.iter().find(|t| t.spec().name == name).expect("valid test fixture").run(&input, workspace)
        }
    }

    #[test]
    fn planning_tasks_track_dependencies_and_reject_cycles_atomically() {
        let workspace = Workspace::new(".");
        let run = runner(&workspace);
        assert!(!run("TaskCreate", json!({"subject":"Build"})).is_error);
        assert!(!run("TaskCreate", json!({"subject":"Verify"})).is_error);
        assert!(!run("TaskUpdate", json!({"task_id":"2","blocked_by":["1"]})).is_error);
        assert!(run("TaskUpdate", json!({"task_id":"1","blocked_by":["2"]})).is_error);
        assert!(run("TaskUpdate", json!({"task_id":"2","status":"completed"})).is_error);
        assert!(!run("TaskUpdate", json!({"task_id":"1","status":"completed"})).is_error);
        assert!(!run("TaskUpdate", json!({"task_id":"2","status":"completed"})).is_error);
    }

    #[cfg(unix)]
    #[test]
    fn a_task_hook_can_refuse_a_new_task_or_its_completion_and_says_why() {
        let dir = tempfile::tempdir().expect("dir");
        let mut workspace = Workspace::new(dir.path());
        let groups = json!({
            "TaskCreated": [{"hooks": [{"type": "command", "command": "grep -q forbidden && { printf 'no forbidden tasks' >&2; exit 2; }; exit 0"}]}],
            "TaskCompleted": [{"hooks": [{"type": "command", "command": "printf 'run the tests first' >&2; exit 2"}]}]
        });
        workspace.hooks = crate::hooks::Hooks::parse(&groups).expect("hooks");
        let run = runner(&workspace);

        let refused = run("TaskCreate", json!({"subject":"a forbidden thing"}));
        let made = run("TaskCreate", json!({"subject":"Build"}));
        let unfinished = run("TaskUpdate", json!({"task_id":"1","status":"completed"}));
        let started = run("TaskUpdate", json!({"task_id":"1","status":"in_progress"}));

        assert!(refused.is_error && refused.content.contains("no forbidden tasks"), "{}", refused.content);
        assert!(!made.is_error && made.content.contains("\"task_id\":\"1\""), "{}", made.content);
        assert!(unfinished.is_error && unfinished.content.contains("run the tests first"), "{}", unfinished.content);
        assert!(!started.is_error, "only completing is hooked: {}", started.content);
        assert!(run("TaskList", json!({})).content.contains("\"status\":\"in_progress\""));
    }

    #[test]
    fn with_a_home_the_list_is_kept_for_the_session_and_survives_the_run() {
        let home = tempfile::tempdir().expect("home");
        let mut workspace = Workspace::new(".");
        workspace.home = Some(home.path().to_path_buf());
        workspace.session_id = "lead-7".into();
        assert!(!runner(&workspace)("TaskCreate", json!({"subject":"Build"})).is_error);

        let listed = runner(&workspace)("TaskList", json!({}));

        assert!(listed.content.contains("\"subject\":\"Build\""), "a new run reads it: {}", listed.content);
        assert!(crate::team::tasks_file(home.path(), "lead-7").is_file());
    }

    #[test]
    fn the_lead_assigning_a_task_to_a_teammate_tells_the_teammate() {
        let home = tempfile::tempdir().expect("home");
        let mut workspace = Workspace::new(".");
        workspace.home = Some(home.path().to_path_buf());
        workspace.session_id = "lead-8".into();
        let team = crate::team::Team::of_session(home.path(), "lead-8");
        team.open("lead-8").expect("team");
        team.join("tester", domain::team::Member::default()).expect("join");
        let run = runner(&workspace);
        assert!(!run("TaskCreate", json!({"subject":"Fix lint"})).is_error);

        let assigned = run("TaskUpdate", json!({"task_id":"1","owner":"tester"}));

        assert!(assigned.content.contains("\"owner\":\"tester\""), "{}", assigned.content);
        let mail = team.unread("tester");
        assert!(matches!(
            Protocol::of(&mail[0].text),
            Some(Protocol::TaskAssignment { task_id, assigned_by, .. }) if task_id == "1" && assigned_by == "team-lead"
        ));
    }
}
