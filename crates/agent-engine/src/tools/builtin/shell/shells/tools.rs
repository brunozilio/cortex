use domain::events::BackgroundChange;
use serde_json::Value;

use super::{lock, refresh, stopped};
use crate::model::ToolSpec;
use crate::tools::{Outcome, Tool, Workspace, clip, field, required};

const SHOWN: usize = 30_000;

#[derive(Debug)]
pub struct BashOutput;

impl Tool for BashOutput {
    fn spec(&self) -> ToolSpec {
        crate::tools::declared("BashOutput", &serde_json::Value::Null)
    }

    fn effects(&self, _input: &Value, _workspace: &Workspace) -> Vec<crate::tools::Effect> {
        Vec::new()
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
        let finished = exit.is_some();
        let state = match exit {
            Some(exit) => format!("finished ({exit})"),
            None => format!("running for {}s", shell.started.elapsed().as_secs()),
        };
        let text = if text.trim().is_empty() { "(no new output)".to_owned() } else { clip(text.trim_end(), SHOWN) };
        let dropped = unread.dropped.map(|dropped| format!("{dropped}\n")).unwrap_or_default();
        if shell.collected() {
            shells.remove(id);
        }
        let outcome = Outcome::ok(format!("[{id}: {state}]\n{dropped}{text}"));
        if finished { outcome.in_the_background(BackgroundChange::Ended(id.to_owned())) } else { outcome }
    }
}

#[derive(Debug)]
pub struct KillShell;

impl Tool for KillShell {
    fn spec(&self) -> ToolSpec {
        crate::tools::declared("KillShell", &serde_json::Value::Null)
    }

    fn effects(&self, input: &Value, workspace: &Workspace) -> Vec<crate::tools::Effect> {
        let id = field(input, "shell_id").unwrap_or_default();
        vec![crate::tools::Effect::Cancel { own: super::started_by(workspace.credentials.client, id) }]
    }

    fn summary(&self, input: &Value) -> String {
        field(input, "shell_id").unwrap_or_default().to_owned()
    }

    fn run(&self, input: &Value, workspace: &Workspace) -> Outcome {
        let id = match required(input, "shell_id") {
            Ok(id) => id,
            Err(error) => return error,
        };
        let ended = BackgroundChange::Ended(id.to_owned());
        if let Some(explained) = crate::tools::watch::stop(workspace.credentials.client, id) {
            return Outcome::ok(format!("Stopped {id} ({explained}).")).in_the_background(ended);
        }
        match stopped(workspace.credentials.client, id) {
            Some(command) => Outcome::ok(format!("Stopped {id} ({command}).")).in_the_background(ended),
            None => Outcome::error(format!("No background command `{id}`.")),
        }
    }
}
