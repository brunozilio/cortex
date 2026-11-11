//! Commands left running in the background (`Bash` with
//! `run_in_background`): a dev server, a watcher, a long build. They
//! outlive the turn that started them; the agent reads what they printed
//! with `BashOutput` and stops them with `KillShell`. Each belongs to the
//! client whose run started it ([`RunCredentials::client`]) and ends when
//! that client leaves ([`kill_client`]).
//!
//! [`RunCredentials::client`]: domain::credentials::RunCredentials::client

use std::collections::HashMap;
use std::io::Read as _;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::Instant;

use serde_json::{Value, json};
use subprocess::Tree;

use super::{Access, Outcome, Tool, Workspace, clip, field, required};
use crate::model::ToolSpec;

mod wait;
mod watch;
pub(crate) use wait::{Waited, wait};
pub(crate) use watch::{Watch, watch};

/// What a shell keeps of its output (the newest part).
const KEPT: usize = 1 << 20;
/// What one `BashOutput` returns at most.
const SHOWN: usize = 30_000;

struct Shell {
    client: u64,
    command: String,
    tree: Tree,
    output: Arc<Mutex<Vec<u8>>>,
    /// How much of its output was dropped from the front so far (for
    /// readers that keep their own place, like a monitor's watch).
    dropped: Arc<std::sync::atomic::AtomicUsize>,
    read: usize,
    started: Instant,
    exit: Option<String>,
    /// The threads reading its stdout and stderr.
    readers: Vec<std::thread::JoinHandle<()>>,
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
    // One pipe for both stdout and stderr, so what it printed keeps its order.
    let (reader, writer) = std::io::pipe()?;
    command.stdout(writer.try_clone()?).stderr(writer);
    let spawned = Tree::spawn(command);
    // The command keeps its copies of the pipe's end until told otherwise;
    // the reader only ends once every writer is gone.
    command.stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    let tree = spawned?;
    let output = Arc::new(Mutex::new(Vec::new()));
    let pipe = boxed(reader);
    let dropped = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let readers = vec![{
        let (output, dropped) = (Arc::clone(&output), Arc::clone(&dropped));
        std::thread::spawn(move || collect(pipe, &output, &dropped))
    }];
    let id = format!("bash_{}", NEXT.fetch_add(1, Ordering::Relaxed));
    let started = Instant::now();
    let shell =
        Shell { client, command: summary.to_owned(), tree, output, dropped, read: 0, started, exit: None, readers };
    lock().insert(id.clone(), shell);
    Ok(id)
}

fn boxed(pipe: impl std::io::Read + Send + 'static) -> Box<dyn std::io::Read + Send> {
    Box::new(pipe)
}

fn collect(mut pipe: Box<dyn std::io::Read + Send>, output: &Mutex<Vec<u8>>, dropped: &std::sync::atomic::AtomicUsize) {
    let mut chunk = [0_u8; 8192];
    while let Ok(read) = pipe.read(&mut chunk) {
        if read == 0 {
            return;
        }
        let mut kept = output.lock().unwrap_or_else(PoisonError::into_inner);
        kept.extend_from_slice(&chunk[..read]);
        dropped.fetch_add(capped(&mut kept), Ordering::Relaxed);
    }
}

/// A command nobody reads keeps only its newest output: past twice
/// [`KEPT`], the oldest goes (`BashOutput` then starts over from what is
/// left).
fn capped(kept: &mut Vec<u8>) -> usize {
    if kept.len() <= 2 * KEPT {
        return 0;
    }
    let drop = kept.len() - KEPT;
    kept.drain(..drop);
    drop
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

/// Stops background command `id` of `client`; whether there was one.
pub fn kill(client: u64, id: &str) -> bool {
    let mut shells = lock();
    if !shells.get(id).is_some_and(|shell| shell.client == client) {
        return false;
    }
    if let Some(mut shell) = shells.remove(id) {
        shell.tree.kill();
        let _ = shell.tree.child().wait();
    }
    true
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
            // Past the end when the reader dropped the oldest output.
            let from = if shell.read > kept.len() { 0 } else { shell.read };
            let new = String::from_utf8_lossy(&kept[from..]).into_owned();
            if kept.len() > KEPT {
                let drop = kept.len() - KEPT;
                kept.drain(..drop);
                shell.dropped.fetch_add(drop, Ordering::Relaxed);
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
mod tests;
