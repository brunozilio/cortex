//! The scheduling tools' schemas and wording.

use super::*;

#[derive(Debug)]
pub struct CronCreate;

impl Tool for CronCreate {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "CronCreate".into(),
            description: "Schedule a prompt to be sent to this session later, on a cron expression in the user's \
                          local time (minute hour day-of-month month day-of-week, e.g. \"*/10 * * * *\", \
                          \"0 9 * * 1-5\"). It fires only while the session is open and idle. Recurring prompts \
                          end after 7 days. Use it when the user asks for something periodic or at a set time."
                .into(),
            schema: json!({
                "type": "object",
                "properties": {
                    "cron": { "type": "string" },
                    "prompt": { "type": "string", "description": "What to send when it fires." },
                    "recurring": { "type": "boolean", "description": "Every match (default) or only the next." }
                },
                "required": ["cron", "prompt"],
                "additionalProperties": false
            }),
        }
    }

    fn access(&self) -> Access {
        // What it schedules is later sent as if the user typed it: asked first.
        Access::Executes
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

/// The shortest and longest wait `ScheduleWakeup` takes, in seconds.
const WAKE: (i64, i64) = (60, 3600);

impl Tool for ScheduleWakeup {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "ScheduleWakeup".into(),
            description: "Send a prompt to this session once, after a delay (60 to 3600 seconds), to pick up \
                          work later: check on a long build, a deploy, or repeat a task at your own pace by \
                          scheduling the next wakeup each time. It fires only while the session is idle."
                .into(),
            schema: json!({
                "type": "object",
                "properties": {
                    "delaySeconds": { "type": "number" },
                    "prompt": { "type": "string" },
                    "reason": { "type": "string", "description": "Why this delay, in one sentence." }
                },
                "required": ["delaySeconds", "prompt"],
                "additionalProperties": false
            }),
        }
    }

    fn access(&self) -> Access {
        // What it schedules is later sent as if the user typed it: asked first.
        Access::Executes
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
        ToolSpec {
            name: "CronList".into(),
            description: "List the prompts scheduled for this session (CronCreate, ScheduleWakeup).".into(),
            schema: json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        }
    }

    fn access(&self) -> Access {
        Access::Reads
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
        ToolSpec {
            name: "CronDelete".into(),
            description: "Remove a scheduled prompt by its id (CronList shows them).".into(),
            schema: json!({
                "type": "object",
                "properties": { "id": { "type": "string" } },
                "required": ["id"],
                "additionalProperties": false
            }),
        }
    }

    fn access(&self) -> Access {
        Access::Reads
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
