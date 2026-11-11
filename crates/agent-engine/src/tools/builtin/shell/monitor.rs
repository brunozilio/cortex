use serde_json::{Value, json};

use super::shells::Watch;
use crate::tools::{Access, Outcome, Tool, Workspace, field, required};
use crate::model::ToolSpec;

#[derive(Debug)]
pub struct Monitor;

impl Tool for Monitor {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "Monitor".into(),
            description: crate::tools::describe("Monitor", &serde_json::Value::Null),
            schema: json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "The command; each line it prints is an event." },
                    "description": { "type": "string", "description": "What it watches, in a few words." }
                },
                "required": ["command"],
                "additionalProperties": false
            }),
        }
    }

    fn access(&self) -> Access {
        Access::Executes
    }

    fn summary(&self, input: &Value) -> String {
        field(input, "description").or(field(input, "command")).unwrap_or_default().to_owned()
    }

    fn run(&self, input: &Value, workspace: &Workspace) -> Outcome {
        let command = match required(input, "command") {
            Ok(command) => command,
            Err(error) => return error,
        };
        let mut shell = super::bash::shell(command);
        shell.current_dir(&workspace.cwd).stdin(std::process::Stdio::null());
        let client = workspace.credentials.client;
        match super::shells::start(client, command, &mut shell) {
            Ok(id) => {
                super::shells::watch(id.clone(), client, Watch::Lines);
                Outcome::ok(format!(
                    "Monitoring as {id}: each line it prints reaches you as an event. Stop it with KillShell."
                ))
            }
            Err(error) => Outcome::error(format!("Could not start the monitor: {error}")),
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;
    use crate::tools::inbox;

    #[test]
    fn each_line_a_monitor_prints_reaches_the_inbox_and_so_does_its_end() {
        let dir = tempfile::tempdir().expect("dir");
        let mut workspace = Workspace::new(dir.path());
        workspace.credentials.client = 8901;
        let started = Monitor.run(&json!({"command": "echo ready; sleep 0.7; echo ERROR boom"}), &workspace);
        assert!(!started.is_error && started.content.contains("Monitoring as bash_"), "{started:?}");
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut seen = Vec::new();
        while !seen.iter().any(|n: &domain::inbox::Notice| n.text.contains("finished")) {
            assert!(Instant::now() < deadline, "{seen:?}");
            std::thread::sleep(Duration::from_millis(100));
            seen.extend(inbox::take(8901));
        }
        let texts: Vec<&str> = seen.iter().map(|n| n.text.as_str()).collect();
        assert!(texts.contains(&"ready"), "{texts:?}");
        assert!(texts.contains(&"ERROR boom"), "{texts:?}");
        assert!(seen.iter().all(|n| n.source.starts_with("monitor bash_")));
    }
}

#[cfg(all(test, unix))]
mod read_tests {
    use std::time::{Duration, Instant};

    use super::*;
    use crate::tools::inbox;

    #[test]
    fn reading_a_monitor_with_bash_output_loses_or_repeats_no_line() {
        let dir = tempfile::tempdir().expect("dir");
        let mut workspace = Workspace::new(dir.path());
        workspace.credentials.client = 8902;
        let started = Monitor.run(&json!({"command": "echo one; sleep 1; echo two; sleep 1; echo three"}), &workspace);
        let id = started.content.split_whitespace().nth(2).expect("id").trim_end_matches(':').to_owned();
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut lines = Vec::new();
        let mut read = false;
        while !lines.iter().any(|text: &String| text.contains("finished")) {
            assert!(Instant::now() < deadline, "{lines:?}");
            std::thread::sleep(Duration::from_millis(100));
            lines.extend(inbox::take(8902).into_iter().map(|notice| notice.text));
            if !read && lines.iter().any(|text| text == "one") {
                super::super::shells::BashOutput.run(&json!({ "bash_id": id }), &workspace);
                read = true;
            }
        }
        let events: Vec<&str> = lines.iter().map(String::as_str).filter(|text| !text.contains("finished")).collect();
        assert_eq!(events, ["one", "two", "three"]);
    }
}
