use serde_json::Value;

use super::{lock, refresh, stopped};
use crate::model::ToolSpec;
use crate::tools::{Access, Outcome, Tool, Workspace, clip, field, required};

const SHOWN: usize = 30_000;

#[derive(Debug)]
pub struct BashOutput;

impl Tool for BashOutput {
    fn spec(&self) -> ToolSpec {
        crate::tools::declared("BashOutput", &serde_json::Value::Null)
    }

    fn access(&self) -> Access {
        crate::tools::declared_access("BashOutput")
    }

    fn summary(&self, input: &Value) -> String {
        field(input, "bash_id").unwrap_or_default().to_owned()
    }

    fn run(&self, input: &Value, workspace: &Workspace) -> Outcome {
        let id = match required(input, "bash_id") {
            Ok(id) => id,
            Err(error) => return error,
        };
        let mut shells = lock();
        let Some(shell) = shells.get_mut(id).filter(|shell| shell.client == workspace.credentials.client) else {
            return Outcome::error(format!("No background command `{id}`."));
        };
        let exit = refresh(shell);
        let unread = shell.take_unread();
        let text = match field(input, "filter") {
            Some(filter) => unread.text.lines().filter(|line| line.contains(filter)).collect::<Vec<_>>().join("\n"),
            None => unread.text,
        };
        let state = match exit {
            Some(exit) => format!("finished ({exit})"),
            None => format!("running for {}s", shell.started.elapsed().as_secs()),
        };
        let text = if text.trim().is_empty() { "(no new output)".to_owned() } else { clip(text.trim_end(), SHOWN) };
        let dropped = unread.dropped.map(|dropped| format!("{dropped}\n")).unwrap_or_default();
        if shell.collected() {
            shells.remove(id);
        }
        Outcome::ok(format!("[{id}: {state}]\n{dropped}{text}"))
    }
}

#[derive(Debug)]
pub struct KillShell;

impl Tool for KillShell {
    fn spec(&self) -> ToolSpec {
        crate::tools::declared("KillShell", &serde_json::Value::Null)
    }

    fn access(&self) -> Access {
        crate::tools::declared_access("KillShell")
    }

    fn summary(&self, input: &Value) -> String {
        field(input, "shell_id").unwrap_or_default().to_owned()
    }

    fn run(&self, input: &Value, workspace: &Workspace) -> Outcome {
        let id = match required(input, "shell_id") {
            Ok(id) => id,
            Err(error) => return error,
        };
        if let Some(explained) = crate::tools::watch::stop(workspace.credentials.client, id) {
            return Outcome::ok(format!("Stopped {id} ({explained})."));
        }
        match stopped(workspace.credentials.client, id) {
            Some(command) => Outcome::ok(format!("Stopped {id} ({command}).")),
            None => Outcome::error(format!("No background command `{id}`.")),
        }
    }
}
