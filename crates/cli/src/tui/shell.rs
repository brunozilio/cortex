use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use platform::{Bounds, Ended};

use super::app::{App, Msg};
use super::transcript::{Entry, ToolState};

const LIMIT: Duration = Duration::from_secs(120);
const KEPT: usize = 30_000;

pub fn start(app: &mut App, command: String) {
    let id = format!("shell-{}", app.transcript.entries.len());
    app.transcript.push(Entry::Tool {
        id: id.clone(),
        name: "!".into(),
        summary: command.clone(),
        state: ToolState::Running,
        output: None,
    });
    let cwd = app.cwd.clone();
    app.spawn(move |_| Msg::Shell { id, result: run(&cwd, &command), command });
}

pub fn finished(app: &mut App, id: &str, command: String, result: Result<(String, bool), String>) {
    let (output, failed) = result.unwrap_or_else(|error| (error, true));
    for entry in &mut app.transcript.entries {
        if let Entry::Tool { id: shown, state, output: out, .. } = entry
            && shown == id
        {
            *state = if failed { ToolState::Failed } else { ToolState::Done };
            *out = Some(output.clone());
        }
    }
    let kept: String = output.chars().take(KEPT).collect();
    app.shell_context.push(format!("$ {command}\n{kept}"));
}

pub fn with_context(context: &mut Vec<String>, prompt: &str) -> String {
    if context.is_empty() {
        return prompt.to_owned();
    }
    let ran = std::mem::take(context).join("\n\n");
    format!("I ran this in the shell myself:\n<shell>\n{ran}\n</shell>\n\n{prompt}")
}

fn run(cwd: &Path, command: &str) -> Result<(String, bool), String> {
    let mut shell = platform::command("sh");
    shell.arg("-c").arg(format!("exec 2>&1\n{command}")).current_dir(cwd);
    let never = AtomicBool::new(false);
    let bounds = Bounds { timeout: LIMIT, max_output: usize::MAX, stop: &never, input: None };
    let finished = platform::bounded::run(&mut shell, bounds).map_err(|error| format!("could not start the shell: {error}"))?;
    let mut out = finished.stdout;
    match finished.ended {
        Ended::Exited(status) => Ok((out.trim_end().to_owned(), !status.success())),
        Ended::TimedOut | Ended::Stopped => {
            out.push_str(&format!("\n[stopped after {} seconds]", LIMIT.as_secs()));
            Ok((out, true))
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn runs_in_the_project_and_says_whether_it_failed() {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::write(dir.path().join("a.txt"), "x").expect("write");
        assert_eq!(run(dir.path(), "ls"), Ok(("a.txt".to_owned(), false)));
        assert_eq!(run(dir.path(), "echo oops >&2; exit 3"), Ok(("oops".to_owned(), true)));
    }

    #[test]
    fn what_ran_goes_once_before_the_next_prompt() {
        let mut context = vec!["$ git status\nclean".to_owned()];
        let sent = with_context(&mut context, "commit it");
        assert!(sent.starts_with("I ran this in the shell myself:\n<shell>\n$ git status\nclean\n</shell>"));
        assert!(sent.ends_with("\n\ncommit it"));
        assert_eq!(with_context(&mut context, "again"), "again");
    }
}
