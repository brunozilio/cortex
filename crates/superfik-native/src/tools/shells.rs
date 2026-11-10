//! Commands left running in the background (`Bash` with
//! `run_in_background`): a dev server, a watcher, a long build. They
//! outlive the turn that started them; the agent reads what they printed
//! with `BashOutput` and stops them with `KillShell`. Each belongs to the
//! client whose run started it ([`RunCredentials::client`]) and ends when
//! that client leaves ([`kill_client`]).
//!
//! [`RunCredentials::client`]: superfik_core::credentials::RunCredentials::client

use std::collections::HashMap;
use std::io::Read as _;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::Instant;

use serde_json::{Value, json};
use superfik_process::Tree;

use super::{Access, Outcome, Tool, Workspace, clip, field, required};
use crate::model::ToolSpec;

/// What a shell keeps of its output (the newest part).
const KEPT: usize = 1 << 20;
/// What one `BashOutput` returns at most.
const SHOWN: usize = 30_000;

struct Shell {
    client: u64,
    command: String,
    tree: Tree,
    output: Arc<Mutex<Vec<u8>>>,
    read: usize,
    started: Instant,
    exit: Option<String>,
}

fn shells() -> &'static Mutex<HashMap<String, Shell>> {
    static SHELLS: OnceLock<Mutex<HashMap<String, Shell>>> = OnceLock::new();
    SHELLS.get_or_init(Mutex::default)
}

fn lock() -> std::sync::MutexGuard<'static, HashMap<String, Shell>> {
    shells().lock().unwrap_or_else(PoisonError::into_inner)
}

/// Starts `command` (already set up, stdout and stderr piped) in the
/// background for `client`; returns its id.
pub(crate) fn start(client: u64, summary: &str, command: &mut std::process::Command) -> std::io::Result<String> {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let mut tree = Tree::spawn(command)?;
    let output = Arc::new(Mutex::new(Vec::new()));
    for pipe in [tree.child().stdout.take().map(boxed), tree.child().stderr.take().map(boxed)].into_iter().flatten() {
        let output = Arc::clone(&output);
        std::thread::spawn(move || collect(pipe, &output));
    }
    let id = format!("bash_{}", NEXT.fetch_add(1, Ordering::Relaxed));
    let shell =
        Shell { client, command: summary.to_owned(), tree, output, read: 0, started: Instant::now(), exit: None };
    lock().insert(id.clone(), shell);
    Ok(id)
}

fn boxed(pipe: impl std::io::Read + Send + 'static) -> Box<dyn std::io::Read + Send> {
    Box::new(pipe)
}

fn collect(mut pipe: Box<dyn std::io::Read + Send>, output: &Mutex<Vec<u8>>) {
    let mut chunk = [0_u8; 8192];
    while let Ok(read) = pipe.read(&mut chunk) {
        if read == 0 {
            return;
        }
        let mut kept = output.lock().unwrap_or_else(PoisonError::into_inner);
        kept.extend_from_slice(&chunk[..read]);
    }
}

/// Stops every background command of a client that left.
pub fn kill_client(client: u64) {
    let mut shells = lock();
    let ids: Vec<String> =
        shells.iter().filter(|(_, shell)| shell.client == client).map(|(id, _)| id.clone()).collect();
    for id in ids {
        if let Some(mut shell) = shells.remove(&id) {
            shell.tree.kill();
            let _ = shell.tree.child().wait();
        }
    }
}

/// The background commands of `client`: id, command, still running.
pub fn list(client: u64) -> Vec<(String, String, bool)> {
    let mut shells = lock();
    let mut listed: Vec<_> = shells
        .iter_mut()
        .filter(|(_, shell)| shell.client == client)
        .map(|(id, shell)| (id.clone(), shell.command.clone(), refresh(shell).is_none()))
        .collect();
    listed.sort();
    listed
}

/// Its exit, once it has one.
fn refresh(shell: &mut Shell) -> Option<String> {
    if shell.exit.is_none()
        && let Ok(Some(status)) = shell.tree.child().try_wait()
    {
        shell.exit = Some(status.code().map_or_else(|| "killed by a signal".to_owned(), |c| format!("exit code {c}")));
    }
    shell.exit.clone()
}

#[derive(Debug)]
pub struct BashOutput;

impl Tool for BashOutput {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "BashOutput".into(),
            description: "Read what a background command (Bash with run_in_background) printed since you last \
                          read it, and whether it is still running. Pass `filter` (a substring) to keep only \
                          the lines that contain it."
                .into(),
            schema: json!({
                "type": "object",
                "properties": {
                    "bash_id": { "type": "string", "description": "The id Bash returned." },
                    "filter": { "type": "string", "description": "Keep only lines containing this." }
                },
                "required": ["bash_id"],
                "additionalProperties": false
            }),
        }
    }

    fn access(&self) -> Access {
        Access::Reads
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
        let text = {
            let mut kept = shell.output.lock().unwrap_or_else(PoisonError::into_inner);
            let new = String::from_utf8_lossy(&kept[shell.read.min(kept.len())..]).into_owned();
            if kept.len() > KEPT {
                let drop = kept.len() - KEPT;
                kept.drain(..drop);
            }
            shell.read = kept.len();
            new
        };
        let text = match field(input, "filter") {
            Some(filter) => text.lines().filter(|line| line.contains(filter)).collect::<Vec<_>>().join("\n"),
            None => text,
        };
        let state = match exit {
            Some(exit) => format!("finished ({exit})"),
            None => format!("running for {}s", shell.started.elapsed().as_secs()),
        };
        let text = if text.trim().is_empty() { "(no new output)".to_owned() } else { clip(text.trim_end(), SHOWN) };
        Outcome::ok(format!("[{id}: {state}]\n{text}"))
    }
}

#[derive(Debug)]
pub struct KillShell;

impl Tool for KillShell {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "KillShell".into(),
            description: "Stop a background command (and everything it started).".into(),
            schema: json!({
                "type": "object",
                "properties": { "shell_id": { "type": "string", "description": "The id Bash returned." } },
                "required": ["shell_id"],
                "additionalProperties": false
            }),
        }
    }

    fn access(&self) -> Access {
        Access::Reads
    }

    fn summary(&self, input: &Value) -> String {
        field(input, "shell_id").unwrap_or_default().to_owned()
    }

    fn run(&self, input: &Value, workspace: &Workspace) -> Outcome {
        let id = match required(input, "shell_id") {
            Ok(id) => id,
            Err(error) => return error,
        };
        let mut shells = lock();
        if !shells.get(id).is_some_and(|shell| shell.client == workspace.credentials.client) {
            return Outcome::error(format!("No background command `{id}`."));
        }
        let Some(mut shell) = shells.remove(id) else {
            return Outcome::error(format!("No background command `{id}`."));
        };
        shell.tree.kill();
        let _ = shell.tree.child().wait();
        Outcome::ok(format!("Stopped {id} ({}).", shell.command))
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::process::Stdio;
    use std::time::Duration;

    use super::*;

    fn workspace(client: u64) -> Workspace {
        let mut workspace = Workspace::new(std::env::temp_dir());
        workspace.credentials.client = client;
        workspace
    }

    fn sh(script: &str) -> std::process::Command {
        let mut command = std::process::Command::new("/bin/sh");
        command.arg("-c").arg(script).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        command
    }

    fn read(id: &str, client: u64) -> String {
        BashOutput.run(&json!({ "bash_id": id }), &workspace(client)).content
    }

    #[test]
    fn a_background_command_is_read_incrementally_and_stopped() {
        let id = start(901, "server", &mut sh("echo ready; sleep 30")).expect("start");
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut first = read(&id, 901);
        while !first.contains("ready") {
            assert!(Instant::now() < deadline, "{first}");
            std::thread::sleep(Duration::from_millis(20));
            first = read(&id, 901);
        }
        assert!(first.contains("running for"), "{first}");
        assert!(read(&id, 901).contains("(no new output)"), "output is read once");
        assert!(read(&id, 902).contains("No background command"), "another client cannot see it");
        assert_eq!(list(901), [(id.clone(), "server".to_owned(), true)]);
        let stopped = KillShell.run(&json!({ "shell_id": id }), &workspace(901));
        assert!(stopped.content.starts_with("Stopped"), "{stopped:?}");
        assert!(list(901).is_empty());
    }

    #[test]
    fn a_finished_command_says_how_and_a_leaving_client_stops_its_own() {
        let done = start(903, "build", &mut sh("echo built; exit 2")).expect("start");
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut text = read(&done, 903);
        while !text.contains("finished") {
            assert!(Instant::now() < deadline, "{text}");
            std::thread::sleep(Duration::from_millis(20));
            text.push_str(&read(&done, 903));
        }
        assert!(text.contains("exit code 2") && text.contains("built"), "{text}");
        start(904, "watch", &mut sh("sleep 30")).expect("start");
        kill_client(904);
        assert!(list(904).is_empty());
    }
}
