//! `Bash`: a command run in the user's shell, in the working directory.

use std::process::Stdio;
use std::time::Duration;

use super::shells::Waited;
use super::{Access, Outcome, Tool, Workspace, clip, field, required};
use crate::model::ToolSpec;
use serde_json::{Value, json};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_TIMEOUT: Duration = Duration::from_secs(600);
const OUTPUT: usize = 30_000;

#[derive(Debug)]
pub struct Bash;

impl Tool for Bash {
    fn spec(&self) -> ToolSpec {
        #[cfg(windows)]
        let shell = subprocess::WindowsShell::detect().name();
        #[cfg(not(windows))]
        let shell = "the user's shell";
        ToolSpec {
            name: "Bash".into(),
            description: format!(
                "Run a shell command in the working directory and return its output (stdout and stderr) and exit \
                 code. Each call starts a fresh shell, so `cd` and variables do not carry over; prefer absolute \
                 paths. The command is stopped after `timeout` milliseconds (default {}, at most {}). Output past \
                 {OUTPUT} characters is cut. Use Read, Glob, Grep, Edit and Write for files instead of cat, find, \
                 grep, sed or echo. Quote paths with spaces. Its output reaches you, not reliably the user. \
                 This computer runs commands with {shell}.\n\n\
                 Background: set `run_in_background` for something that keeps running (a dev server, a \
                 watcher, a long build). It returns an id at once; read what it printed with BashOutput and \
                 stop it with KillShell. It keeps running across turns until stopped or until the user \
                 leaves. Never use `&` or `nohup` for this.\n\n\
                 Git:\n\
                 - Interactive flags (`git rebase -i`, `git add -i`) do not work here: there is no terminal.\n\
                 - Use the `gh` CLI for GitHub (pull requests, issues, the API).\n\
                 - GitHub notifies the owner of any `@name` written in a pull request, issue or comment. Write a \
                 bare `@name` only for a real login you mean to notify; put roles, placeholders and code \
                 (`@reviewer`, `@types/node`) in backticks or drop the `@`.\n\
                 - Commit or push only when the user asks. On the default branch, create a branch first.\n\
                 - Never force-push, rewrite published history or skip hooks unless the user asks for that.",
                DEFAULT_TIMEOUT.as_millis(),
                MAX_TIMEOUT.as_millis()
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
        let mut shell = shell(command);
        shell.current_dir(&workspace.cwd).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        if input.get("run_in_background").and_then(Value::as_bool) == Some(true) {
            return match super::shells::start(workspace.credentials.client, command, &mut shell) {
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
        let id = match super::shells::start(workspace.credentials.client, command, &mut shell) {
            Ok(id) => id,
            Err(error) => return Outcome::error(format!("Could not start the shell: {error}")),
        };
        match super::shells::wait(&id, timeout, &workspace.stop, &workspace.detach) {
            Waited::Exited { output, success: true, .. } => {
                let text = clip(&output, OUTPUT);
                Outcome::ok(if text.is_empty() { "(no output)".to_owned() } else { text })
            }
            Waited::Exited { output, code, .. } => {
                let code = code.map_or_else(|| "a signal".to_owned(), |code| code.to_string());
                Outcome::error(format!("{}\n(exit code {code})", clip(&output, OUTPUT)).trim_start().to_owned())
            }
            Waited::Detached => {
                super::shells::watch(id.clone(), workspace.credentials.client, super::shells::Watch::End);
                Outcome::ok(format!(
                    "The user moved this command to the background, where it goes on as {id}. Read its output \
                     with BashOutput and stop it with KillShell; you will be told when it ends."
                ))
            }
            Waited::Stopped(output) => Outcome::error(format!("{}\n(cancelled)", clip(&output, OUTPUT))),
            Waited::TimedOut(output) => Outcome::error(
                format!("{}\n(stopped after {} ms)", clip(&output, OUTPUT), timeout.as_millis())
                    .trim_start()
                    .to_owned(),
            ),
        }
    }
}

#[cfg(unix)]
pub(super) fn shell(command: &str) -> std::process::Command {
    let program = std::env::var("SHELL").ok().filter(|shell| !shell.is_empty()).unwrap_or_else(|| "/bin/sh".into());
    let mut shell = subprocess::command(program);
    shell.arg("-c").arg(command);
    shell
}

#[cfg(windows)]
pub(super) fn shell(command: &str) -> std::process::Command {
    subprocess::WindowsShell::detect().command(command)
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

    #[test]
    fn stops_at_the_timeout() {
        let workspace = Workspace::new(std::env::temp_dir());
        let started = Instant::now();
        let outcome = Bash.run(&json!({"command": "sleep 5", "timeout": 200}), &workspace);
        assert!(outcome.is_error && outcome.content.contains("stopped after 200 ms"), "{outcome:?}");
        assert!(started.elapsed() < Duration::from_secs(3));
    }
}
