//! `Monitor`: waits until a condition holds, running a check command every
//! few seconds until it succeeds (a server answering, a file appearing, a
//! build finishing) or the time is up. The wait never spins the model.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::shells::Waited;
use super::{Access, Outcome, Tool, Workspace, clip, field, required};
use crate::model::ToolSpec;

/// The longest wait.
const MAX_WAIT: u64 = 3_600;

#[derive(Debug)]
pub struct Monitor;

impl Tool for Monitor {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "Monitor".into(),
            description: "Wait until a condition holds: run `until` (a shell command) every `every_seconds` (5 by \
                          default) until it exits successfully, or until `timeout_seconds` (600 by default, at most \
                          3600). Use it instead of polling with repeated Bash calls; it returns the check's last \
                          output."
                .into(),
            schema: json!({
                "type": "object",
                "properties": {
                    "until": { "type": "string", "description": "The check, e.g. `curl -sf localhost:3000/health`." },
                    "every_seconds": { "type": "integer", "minimum": 1, "maximum": 300 },
                    "timeout_seconds": { "type": "integer", "minimum": 1, "maximum": MAX_WAIT }
                },
                "required": ["until"],
                "additionalProperties": false
            }),
        }
    }

    fn access(&self) -> Access {
        Access::Executes
    }

    fn summary(&self, input: &Value) -> String {
        field(input, "until").unwrap_or_default().to_owned()
    }

    fn run(&self, input: &Value, workspace: &Workspace) -> Outcome {
        let check = match required(input, "until") {
            Ok(check) => check,
            Err(error) => return error,
        };
        let every = Duration::from_secs(input.get("every_seconds").and_then(Value::as_u64).unwrap_or(5).clamp(1, 300));
        let limit =
            Duration::from_secs(input.get("timeout_seconds").and_then(Value::as_u64).unwrap_or(600).min(MAX_WAIT));
        let started = Instant::now();
        let mut last = String::new();
        let mut tries = 0;
        while started.elapsed() < limit {
            if workspace.stop.load(Ordering::Relaxed) {
                return Outcome::error("The run was cancelled.");
            }
            tries += 1;
            let mut shell = std::process::Command::new("/bin/sh");
            shell.arg("-c").arg(check).current_dir(&workspace.cwd).stdin(std::process::Stdio::null());
            let id = match super::shells::start(workspace.credentials.client, check, &mut shell) {
                Ok(id) => id,
                Err(error) => return Outcome::error(format!("Could not run the check: {error}")),
            };
            match super::shells::wait(&id, every.max(Duration::from_secs(30)), &workspace.stop, &AtomicBool::new(false))
            {
                Waited::Exited { output, success: true, .. } => {
                    let waited = started.elapsed().as_secs();
                    return Outcome::ok(format!(
                        "The condition holds after {waited}s ({tries} checks):\n{}",
                        clip(&output, 4_000)
                    ));
                }
                Waited::Exited { output, .. } | Waited::TimedOut(output) => last = output,
                Waited::Stopped(_) => return Outcome::error("The run was cancelled."),
                Waited::Detached => {}
            }
            let next = Instant::now() + every;
            while Instant::now() < next {
                if workspace.stop.load(Ordering::Relaxed) {
                    return Outcome::error("The run was cancelled.");
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        Outcome::error(format!(
            "The condition did not hold within {}s ({tries} checks). The last check printed:\n{}",
            limit.as_secs(),
            clip(&last, 4_000)
        ))
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn waits_until_the_check_succeeds_or_gives_up() {
        let dir = tempfile::tempdir().expect("dir");
        let workspace = Workspace::new(dir.path());
        let flag = dir.path().join("ready");
        let maker = std::thread::spawn({
            let flag = flag.clone();
            move || {
                std::thread::sleep(Duration::from_millis(1_500));
                std::fs::write(flag, "ok").expect("write");
            }
        });
        let ok = Monitor.run(&json!({"until": "cat ready", "every_seconds": 1, "timeout_seconds": 10}), &workspace);
        maker.join().expect("maker");
        assert!(
            !ok.is_error && ok.content.starts_with("The condition holds after") && ok.content.ends_with("ok"),
            "{ok:?}"
        );
        let never = Monitor
            .run(&json!({"until": "echo not yet; exit 1", "every_seconds": 1, "timeout_seconds": 2}), &workspace);
        assert!(never.is_error && never.content.contains("not yet"), "{never:?}");
    }
}
