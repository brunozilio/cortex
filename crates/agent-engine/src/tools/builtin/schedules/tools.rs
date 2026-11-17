use super::*;

#[derive(Debug)]
pub struct CronCreate;

impl Tool for CronCreate {
    fn spec(&self) -> ToolSpec {
        crate::tools::declared("CronCreate", &serde_json::Value::Null)
    }

    fn effects(&self, _input: &Value, _workspace: &Workspace) -> Vec<crate::tools::Effect> {
        vec![crate::tools::Effect::Spawn(crate::tools::SpawnKind::Schedule)]
    }

    fn summary(&self, input: &Value) -> String {
        field(input, "cron").unwrap_or_default().to_owned()
    }

    fn run(&self, input: &Value, workspace: &Workspace) -> Outcome {
        let (expr, prompt) = match (required(input, "cron"), required(input, "prompt")) {
            (Ok(expr), Ok(prompt)) => (expr, prompt),
            (Err(error), _) | (_, Err(error)) => return error,
        };
        let recurring = input["recurring"].as_bool().unwrap_or(true);
        let when = When::Cron { expr: expr.to_owned(), recurring };
        match add(workspace.credentials.client, prompt, when, now()) {
            Ok(scheduled) => Outcome::ok(format!("Scheduled {}", shown(&scheduled))),
            Err(error) => Outcome::error(error),
        }
    }
}

#[derive(Debug)]
pub struct ScheduleWakeup;

const WAKE: (i64, i64) = (60, 3600);

impl Tool for ScheduleWakeup {
    fn spec(&self) -> ToolSpec {
        crate::tools::declared("ScheduleWakeup", &serde_json::Value::Null)
    }

    fn effects(&self, _input: &Value, _workspace: &Workspace) -> Vec<crate::tools::Effect> {
        vec![crate::tools::Effect::Spawn(crate::tools::SpawnKind::Schedule)]
    }

    fn summary(&self, input: &Value) -> String {
        field(input, "reason").unwrap_or_default().to_owned()
    }

    fn run(&self, input: &Value, workspace: &Workspace) -> Outcome {
        let prompt = match required(input, "prompt") {
            Ok(prompt) => prompt,
            Err(error) => return error,
        };
        let Some(delay) = input["delaySeconds"].as_f64() else {
            return Outcome::error("Missing required parameter `delaySeconds`.");
        };
        let delay = (delay as i64).clamp(WAKE.0, WAKE.1);
        match add(workspace.credentials.client, prompt, When::At(now() + delay), now()) {
            Ok(scheduled) => Outcome::ok(format!("Wakeup in {delay}s: {}", shown(&scheduled))),
            Err(error) => Outcome::error(error),
        }
    }
}

#[derive(Debug)]
pub struct CronList;

impl Tool for CronList {
    fn spec(&self) -> ToolSpec {
        crate::tools::declared("CronList", &serde_json::Value::Null)
    }

    fn effects(&self, _input: &Value, _workspace: &Workspace) -> Vec<crate::tools::Effect> {
        Vec::new()
    }

    fn summary(&self, _input: &Value) -> String {
        String::new()
    }

    fn run(&self, _input: &Value, workspace: &Workspace) -> Outcome {
        let listed: Vec<String> = list(workspace.credentials.client).iter().map(shown).collect();
        if listed.is_empty() { Outcome::ok("Nothing is scheduled.") } else { Outcome::ok(listed.join("\n")) }
    }
}

#[derive(Debug)]
pub struct CronDelete;

impl Tool for CronDelete {
    fn spec(&self) -> ToolSpec {
        crate::tools::declared("CronDelete", &serde_json::Value::Null)
    }

    fn effects(&self, _input: &Value, _workspace: &Workspace) -> Vec<crate::tools::Effect> {
        vec![crate::tools::Effect::Cancel { own: true }]
    }

    fn summary(&self, input: &Value) -> String {
        field(input, "id").unwrap_or_default().to_owned()
    }

    fn run(&self, input: &Value, workspace: &Workspace) -> Outcome {
        let id = match required(input, "id") {
            Ok(id) => id,
            Err(error) => return error,
        };
        if remove(workspace.credentials.client, id) {
            Outcome::ok(format!("Removed {id}."))
        } else {
            Outcome::error(format!("Nothing scheduled is called {id}."))
        }
    }
}
