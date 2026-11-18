use std::process::Stdio;
use std::time::Duration;

use super::shaped::shaped;
use super::shells::Waited;
use crate::model::ToolSpec;
use crate::tools::{Outcome, Tool, Workspace, field, required};
use domain::events::BackgroundChange;
use serde_json::{Value, json};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_TIMEOUT: Duration = Duration::from_secs(600);
const OUTPUT: usize = domain::output::DEFAULT_LIMIT;

#[derive(Debug)]
pub struct Bash;

impl Tool for Bash {
    fn spec(&self) -> ToolSpec {
        let shell = platform::shell_name();
        crate::tools::declared(
            crate::tools::names::BASH,
            &json!({ "default_timeout": DEFAULT_TIMEOUT.as_millis(), "max_timeout": MAX_TIMEOUT.as_millis(), "OUTPUT": OUTPUT, "shell": shell }),
        )
    }

    fn effects(&self, input: &Value, _workspace: &Workspace) -> Vec<crate::tools::Effect> {
        let unsandboxed = input["unsandboxed"].as_bool() == Some(true);
        vec![crate::tools::Effect::exec(field(input, "command").unwrap_or_default(), unsandboxed)]
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
        let mut shell = if input["unsandboxed"].as_bool() == Some(true) {
            platform::shell(command)
        } else {
            shell(command, workspace)
        };
        shell.current_dir(&workspace.cwd).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let inherited = std::env::var("GIT_CONFIG_COUNT").ok();
        shell.envs(platform::git::environment(&workspace.cwd, inherited.as_deref(), git_run(command, workspace)));
        super::shaped::plain(&mut shell, workspace);
        if input.get("run_in_background").and_then(Value::as_bool) == Some(true) {
            return match super::shells::start(workspace, command, &mut shell) {
                Ok(id) => {
                    super::shells::watch(id.clone(), workspace.credentials.client, super::shells::Watch::End);
                    Outcome::ok(format!(
                        "Started in the background as {id}. Read its output with BashOutput and stop it with \
                         KillShell; you will be told when it ends."
                    ))
                    .in_the_background(BackgroundChange::Started(id))
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
        let not_kept =
            watched.map(|watched| watched.after(workspace)).and_then(|skipped| super::changes::not_kept(&skipped));
        let mut outcome = match waited {
            Waited::Exited { output, success: true, .. } => {
                let text = limited(&shaped(command, &output, workspace), workspace);
                Outcome::ok(if text.is_empty() { "(no output)".to_owned() } else { text })
            }
            Waited::Exited { output, code, .. } => {
                let code = code.map_or_else(|| "a signal".to_owned(), |code| code.to_string());
                let refused = workspace.sandboxed() && domain::sandbox::blocked(&output);
                let hint = if refused { format!("\n{}", domain::sandbox::hint(brand::DIR)) } else { String::new() };
                Outcome::error(
                    format!("{}\n(exit code {code}){hint}", limited(&shaped(command, &output, workspace), workspace))
                        .trim_start()
                        .to_owned(),
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
                Outcome::error(crate::tools::stopped::while_running(&limited(&output, workspace)))
            }
            Waited::TimedOut(output) => Outcome::error(
                format!("{}\n(stopped after {} ms)", limited(&output, workspace), timeout.as_millis())
                    .trim_start()
                    .to_owned(),
            ),
        };
        for note in kept.note().into_iter().chain(not_kept) {
            outcome.content = format!("{}\n{note}", outcome.content);
        }
        outcome
    }
}

fn limited(text: &str, workspace: &Workspace) -> String {
    let limit = workspace.policy.bash_output;
    if !limit.exceeded_by(text) {
        return text.to_owned();
    }
    let saved = super::shaped::kept_whole(text, workspace).map(|path| path.display().to_string());
    limit.cut(text, saved.as_deref())
}

fn git_run(command: &str, workspace: &Workspace) -> platform::git::Run {
    let home = workspace.home.as_deref().and_then(std::path::Path::to_str);
    if domain::command_policy::runs_as_a_read(command, home) {
        platform::git::Run::ReadOnly
    } else {
        platform::git::Run::Approved
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

    #[test]
    fn a_command_s_change_tracking_runs_nothing_the_repository_s_config_names() {
        let hostile = crate::hostile_repository::Hostile::new();
        let mut workspace = Workspace::new(hostile.path());
        workspace.services.files = Some(std::sync::Arc::default());

        let ran = Bash.run(&json!({"command": "echo three > a.txt && echo new > b.txt"}), &workspace);

        assert!(!ran.is_error, "{ran:?}");
        let recorded = workspace.services.files.as_ref().expect("files").take().files;
        assert!(recorded.keys().any(|path| path.ends_with("a.txt")), "the change was still tracked: {recorded:?}");
        assert_eq!(hostile.ran(), None, "the repository's fsmonitor, filter or hook ran");
        assert!(hostile.plain_git_runs_it(), "the same repository does run them through plain git");
    }

    #[test]
    fn the_git_reads_the_agent_runs_without_asking_run_nothing_the_repository_s_config_names() {
        let hostile = crate::hostile_repository::Hostile::new();
        let workspace = Workspace::new(hostile.path());

        for command in ["git status", "git diff", "git log -p", "git show HEAD", "git diff HEAD --stat"] {
            let ran = Bash.run(&json!({ "command": command }), &workspace);
            assert!(!ran.is_error, "{command}: {ran:?}");
        }

        assert_eq!(hostile.ran(), None, "the repository's fsmonitor or filter ran");
        assert!(hostile.plain_git_runs_it(), "the same repository does run them through plain git");
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

        workspace.policy.sandbox = Some(domain::sandbox::Sandbox::default());
        let inside = Bash.run(&json!({"command": "echo kept > kept.txt && cat kept.txt"}), &workspace);
        assert_eq!(inside, Outcome::ok("kept"));
        let refused = Bash.run(&grant, &workspace);
        assert!(refused.is_error && refused.content.contains("sandbox.allowWrite"), "{refused:?}");
        assert_eq!(std::fs::read_to_string(&settings).expect("read"), "{}");

        let failed = Bash.run(&json!({"command": "exit 3"}), &workspace);
        assert_eq!(failed, Outcome::error("(exit code 3)"));
    }

    #[test]
    fn with_output_cleanup_a_command_runs_without_colours_and_its_escape_codes_are_dropped() {
        let mut workspace = Workspace::new(std::env::temp_dir());
        let command = json!({"command": "printf '\\033[31m%s\\033[0m' \"$NO_COLOR\""});
        assert!(Bash.run(&command, &workspace).content.starts_with("\u{1b}[31m"));

        workspace.policy.output.cleanup = true;
        assert_eq!(Bash.run(&command, &workspace), Outcome::ok("1"));
    }

    #[test]
    fn a_long_output_is_cut_to_the_limit_with_a_first_line_saying_how_much_and_where_the_whole_is() {
        let mut workspace = Workspace::new(std::env::temp_dir());
        workspace.session_id = format!("bash-limit-{}", std::process::id());
        workspace.policy.bash_output = domain::output::OutputLimit::of(1_000);
        let outcome = Bash.run(&json!({"command": "seq 1 2000"}), &workspace);
        assert!(!outcome.is_error, "{}", outcome.content);
        let (note, rest) = outcome.content.split_once('\n').expect("note");
        assert!(note.starts_with("[Output cut: 8892 characters in 2000 lines, over the limit of 1000"), "{note}");
        let path = note
            .split_once("The full output is in ")
            .and_then(|(_, after)| after.strip_suffix(": Read or Grep it for the rest.]"))
            .expect("the path");
        let whole = std::fs::read_to_string(path).expect("saved");
        let _ = std::fs::remove_dir_all(crate::tools::persisted::results_dir(&workspace.session_id));
        assert_eq!(whole.lines().count(), 2000);
        assert!(rest.starts_with("1\n2\n3\n") && rest.ends_with("\n1999\n2000"), "{rest}");
        assert!(rest.chars().count() < 1_100);

        let failed = Bash.run(&json!({"command": "seq 1 2000; exit 4"}), &workspace);
        assert!(failed.is_error && failed.content.starts_with("[Output cut: "), "{}", failed.content);
        assert!(failed.content.ends_with("\n2000\n(exit code 4)"), "{}", failed.content);
        let _ = std::fs::remove_dir_all(crate::tools::persisted::results_dir(&workspace.session_id));
        assert_eq!(Bash.run(&json!({"command": "seq 1 3"}), &workspace).content, "1\n2\n3");
    }

    #[test]
    fn stops_at_the_timeout() {
        let workspace = Workspace::new(std::env::temp_dir());
        let started = Instant::now();
        let outcome = Bash.run(&json!({"command": "sleep 60", "timeout": 200}), &workspace);
        assert!(outcome.is_error && outcome.content.contains("stopped after 200 ms"), "{outcome:?}");
        assert!(started.elapsed() < Duration::from_secs(30), "the command ran on: {:?}", started.elapsed());
    }

    #[test]
    fn a_command_started_in_the_background_says_so_beside_its_text() {
        let mut workspace = Workspace::new(std::env::temp_dir());
        workspace.credentials.client = 921;
        let started = Bash.run(&json!({"command": "sleep 30", "run_in_background": true}), &workspace);
        let Some(BackgroundChange::Started(id)) = started.background.as_deref().cloned() else {
            panic!("no background start in {started:?}")
        };
        assert!(started.content.contains(&id), "{started:?}");
        assert!(super::super::shells::kill(921, &id));
        assert_eq!(Bash.run(&json!({"command": "true"}), &workspace).background, None);
    }
}
