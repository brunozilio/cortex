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
    app.spawn(move |sdk| Msg::Shell { id, result: run(sdk, &cwd, &command), command });
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
    app.shell_context.push(domain::conversation::shell_note(&command, &output));
}

pub use domain::conversation::with_shell as with_context;

fn run(sdk: &sdk::Sdk, cwd: &Path, command: &str) -> Result<(String, bool), String> {
    let ran = sdk.feature("shell", serde_json::json!({ "run": command, "path": cwd })).map_err(|e| e.to_string())?;
    Ok((ran["output"].as_str().unwrap_or_default().to_owned(), ran["failed"] == true))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn runs_in_the_project_and_says_whether_it_failed() {
        let (data, dir) = (tempfile::tempdir().expect("data"), tempfile::tempdir().expect("dir"));
        std::fs::write(dir.path().join("a.txt"), "x").expect("write");
        let engine = agent_engine::fake::FakeEngine::new(Vec::new(), Vec::new());
        let sdk = super::super::tests::sdk(&engine, data.path());
        assert_eq!(run(&sdk, dir.path(), "ls"), Ok(("a.txt".to_owned(), false)));
        assert_eq!(run(&sdk, dir.path(), "echo oops >&2; exit 3"), Ok(("oops".to_owned(), true)));
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
