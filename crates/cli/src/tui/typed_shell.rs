use std::path::Path;

use super::app::{App, Msg};
use super::transcript::{Entry, ToolState};

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
    app.spawn(move |client| Msg::Shell { id, result: run(client, &cwd, &command), command });
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
    app.prompt.shell_context.push(domain::conversation::shell_note(&command, &output));
}

pub use domain::conversation::with_shell as with_context;

fn run(client: &app::Client, cwd: &Path, command: &str) -> Result<(String, bool), String> {
    client.run_typed(cwd, command).map_err(|error| error.to_string())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn runs_in_the_project_and_says_whether_it_failed() {
        let (data, dir) = (tempfile::tempdir().expect("data"), tempfile::tempdir().expect("dir"));
        std::fs::write(dir.path().join("a.txt"), "x").expect("write");
        let engine = agent_engine::fake::FakeEngine::new(Vec::new(), Vec::new());
        let client = super::super::tests::client(&engine, data.path());
        assert_eq!(run(&client, dir.path(), "ls"), Ok(("a.txt".to_owned(), false)));
        assert_eq!(run(&client, dir.path(), "echo oops >&2; exit 3"), Ok(("oops".to_owned(), true)));
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
