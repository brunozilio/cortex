use std::io::Read as _;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

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
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(format!("exec 2>&1\n{command}"))
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|error| format!("could not start the shell: {error}"))?;
    let mut stdout = child.stdout.take().ok_or("no output")?;
    let reader = std::thread::spawn(move || {
        let mut out = String::new();
        let _ = stdout.read_to_string(&mut out);
        out
    });
    let deadline = Instant::now() + LIMIT;
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            break Some(status);
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let mut out = reader.join().unwrap_or_default();
    match status {
        Some(status) => Ok((out.trim_end().to_owned(), !status.success())),
        None => {
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
