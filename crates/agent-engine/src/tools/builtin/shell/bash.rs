use std::process::Stdio;
use std::time::Duration;

use super::clip_middle;
use super::shells::Waited;
use crate::model::ToolSpec;
use crate::tools::{Access, Outcome, Tool, Workspace, field, required};
use serde_json::{Value, json};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_TIMEOUT: Duration = Duration::from_secs(600);
const OUTPUT: usize = 2_000_000;

#[derive(Debug)]
pub struct Bash;

impl Tool for Bash {
    fn spec(&self) -> ToolSpec {
        let shell = platform::shell_name();
        ToolSpec {
            name: "Bash".into(),
            description: crate::tools::describe(
                "Bash",
                &serde_json::json!({ "default_timeout": DEFAULT_TIMEOUT.as_millis(), "max_timeout": MAX_TIMEOUT.as_millis(), "OUTPUT": OUTPUT, "shell": shell }),
            ),
            schema: json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "The command to run." },
                    "timeout": { "type": "integer", "description": "Milliseconds before it is stopped." },
                    "run_in_background": { "type": "boolean", "description": "Leave it running and return its id at once." },
                    "description": { "type": "string", "description": "What the command does, in plain active words the user reads, often without seeing the command: do not echo its text, flags or paths. Five to ten words for simple commands (`git status` → \"Show working tree status\"); more context for pipes or obscure flags (`git reset --hard origin/main` → \"Discard all local changes and match remote main\"). Never call a command \"complex\" or \"risky\": say what it does." }
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
        field(input, "command").unwrap_or_default().to_owned()
    }

    fn run(&self, input: &Value, workspace: &Workspace) -> Outcome {
        let command = match required(input, "command") {
            Ok(command) => command,
            Err(error) => return error,
        };
        let timeout = input
            .get("timeout")
            .and_then(Value::as_u64)
            .map_or(DEFAULT_TIMEOUT, Duration::from_millis)
            .min(MAX_TIMEOUT);
        let kept = super::snapshot::keep(workspace, command);
        let mut shell = shell(command, workspace);
        shell.current_dir(&workspace.cwd).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        if input.get("run_in_background").and_then(Value::as_bool) == Some(true) {
            return match super::shells::start(workspace, command, &mut shell) {
                Ok(id) => {
                    super::shells::watch(id.clone(), workspace.credentials.client, super::shells::Watch::End);
                    Outcome::ok(format!(
                        "Started in the background as {id}. Read its output with BashOutput and stop it with \
                         KillShell; you will be told when it ends."
                    ))
                }
                Err(error) => Outcome::error(format!("Could not start the shell: {error}")),
            };
        }
        let watched = super::changes::before(workspace);
        let id = match super::shells::start(workspace, command, &mut shell) {
            Ok(id) => id,
            Err(error) => return Outcome::error(format!("Could not start the shell: {error}")),
        };
        let waited = super::shells::wait(&id, timeout, &workspace.stop, &workspace.detach);
        if let Some(watched) = watched {
            watched.after(workspace);
        }
        let mut outcome = match waited {
            Waited::Exited { output, success: true, .. } => {
                let text = clip_middle(&output, OUTPUT);
                Outcome::ok(if text.is_empty() { "(no output)".to_owned() } else { text })
            }
            Waited::Exited { output, code, .. } => {
                let code = code.map_or_else(|| "a signal".to_owned(), |code| code.to_string());
                let refused = workspace.sandboxed() && domain::sandbox::blocked(&output);
                let hint = if refused { format!("\n{}", domain::sandbox::hint(brand::DIR)) } else { String::new() };
                Outcome::error(
                    format!("{}\n(exit code {code}){hint}", clip_middle(&output, OUTPUT)).trim_start().to_owned(),
                )
            }
            Waited::Detached => {
                super::shells::watch(id.clone(), workspace.credentials.client, super::shells::Watch::End);
                Outcome::ok(format!(
                    "The user moved this command to the background, where it goes on as {id}. Read its output \
                     with BashOutput and stop it with KillShell; you will be told when it ends."
                ))
            }
            Waited::Stopped(output) => {
                Outcome::error(crate::tools::stopped::while_running(&clip_middle(&output, OUTPUT)))
            }
            Waited::TimedOut(output) => Outcome::error(
                format!("{}\n(stopped after {} ms)", clip_middle(&output, OUTPUT), timeout.as_millis())
                    .trim_start()
                    .to_owned(),
            ),
        };
        if let Some(note) = kept.note() {
            outcome.content = format!("{}\n{note}", outcome.content);
        }
        outcome
    }
}

pub(super) fn shell(command: &str, workspace: &Workspace) -> std::process::Command {
    workspace.confined(platform::shell(command))
}

#[cfg(all(test, unix))]
mod tests {
    use std::time::Instant;

    use super::*;

    #[test]
    fn runs_in_the_working_directory_and_reports_failures() {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::write(dir.path().join("here.txt"), "").expect("write");
        let workspace = Workspace::new(dir.path());
        assert_eq!(Bash.run(&json!({"command": "ls"}), &workspace), Outcome::ok("here.txt"));

        let failed = Bash.run(&json!({"command": "echo out; echo err >&2; exit 3"}), &workspace);
        assert_eq!(failed, Outcome::error("out\nerr\n(exit code 3)"));
        assert_eq!(Bash.run(&json!({"command": "true"}), &workspace).content, "(no output)");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn a_sandboxed_command_writes_in_the_project_only_and_is_told_why_it_failed() {
        let dir = tempfile::tempdir().expect("dir");
        let settings = dir.path().join(brand::DIR).join("settings.json");
        std::fs::create_dir(dir.path().join(brand::DIR)).expect("folder");
        let mut workspace = Workspace::new(dir.path());
        let grant = json!({"command": format!("echo granted > {}/settings.json", brand::DIR)});
        assert!(!Bash.run(&grant, &workspace).is_error, "nothing confines a run without the sandbox");
        std::fs::write(&settings, "{}").expect("settings");

        workspace.policy.sandbox = Some(domain::sandbox::Sandbox { allow_write: Vec::new(), network: true });
        let inside = Bash.run(&json!({"command": "echo kept > kept.txt && cat kept.txt"}), &workspace);
        assert_eq!(inside, Outcome::ok("kept"));
        let refused = Bash.run(&grant, &workspace);
        assert!(refused.is_error && refused.content.contains("sandbox.allowWrite"), "{refused:?}");
        assert_eq!(std::fs::read_to_string(&settings).expect("read"), "{}");

        let failed = Bash.run(&json!({"command": "exit 3"}), &workspace);
        assert_eq!(failed, Outcome::error("(exit code 3)"));
    }

    #[test]
    fn stops_at_the_timeout() {
        let workspace = Workspace::new(std::env::temp_dir());
        let started = Instant::now();
        let outcome = Bash.run(&json!({"command": "sleep 5", "timeout": 200}), &workspace);
        assert!(outcome.is_error && outcome.content.contains("stopped after 200 ms"), "{outcome:?}");
        assert!(started.elapsed() < Duration::from_secs(3));
    }
}
