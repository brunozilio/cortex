use std::time::Duration;

use platform::bounded::{Bounds, Ended};
use serde_json::{Value, json};

use crate::tools::{Access, Outcome, Tool, Workspace, field, required};
use crate::model::ToolSpec;

#[derive(Debug)]
pub struct PushNotification;

pub(crate) fn command(title: &str, message: &str) -> (String, Vec<String>, Vec<(String, String)>) {
    platform::notification(title, message)
}

impl Tool for PushNotification {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "PushNotification".into(),
            description: crate::tools::describe("PushNotification", &serde_json::Value::Null),
            schema: json!({
                "type": "object",
                "properties": {
                    "message": { "type": "string" },
                    "title": { "type": "string", "description": concat!(brand::name!(), " by default.") }
                },
                "required": ["message"],
                "additionalProperties": false
            }),
        }
    }

    fn access(&self) -> Access {
        Access::Reads
    }

    fn summary(&self, input: &Value) -> String {
        field(input, "message").unwrap_or_default().to_owned()
    }

    fn run(&self, input: &Value, workspace: &Workspace) -> Outcome {
        let message = match required(input, "message") {
            Ok(message) => crate::tools::clip(message, 300),
            Err(error) => return error,
        };
        let title = field(input, "title").filter(|title| !title.trim().is_empty()).unwrap_or(brand::NAME);
        let title = crate::tools::clip(title, 80);
        let (program, args, env) = command(&title, &message);
        let mut process = platform::command(&program);
        process.args(&args).envs(env);
        let bounds = Bounds { timeout: Duration::from_secs(15), max_output: 4_096, stop: &workspace.stop, input: None };
        match platform::bounded::run(&mut process, bounds) {
            Ok(done) if matches!(done.ended, Ended::Exited(status) if status.success()) => Outcome::ok("Notified."),
            Ok(done) => Outcome::error(format!("The notification failed: {}", done.stderr.trim())),
            Err(_) => Outcome::error(format!("Cannot show notifications here ({program} is missing).")),
        }
    }
}
