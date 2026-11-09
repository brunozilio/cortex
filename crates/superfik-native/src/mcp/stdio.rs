//! An MCP server run as a process: JSON-RPC messages one per line on its
//! standard input and output. The process (and whatever it starts) ends
//! with the run.

use std::io::{BufRead, BufReader, Write};
use std::process::{ChildStdin, Stdio};
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::time::Duration;

use serde_json::Value;
use superfik_process::Tree;

use super::Transport;

const ANSWER_WAIT: Duration = Duration::from_secs(600);

pub struct StdioTransport {
    name: String,
    requests: Mutex<()>,
    tree: Mutex<Tree>,
    input: Mutex<ChildStdin>,
    lines: Mutex<Receiver<String>>,
}

impl std::fmt::Debug for StdioTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StdioTransport").field("name", &self.name).finish_non_exhaustive()
    }
}

impl StdioTransport {
    pub fn spawn(name: &str, command: &str, args: &[String], env: &[(String, String)]) -> Result<Self, String> {
        Self::spawn_isolated(name, command, args, env, None)
    }
    pub fn spawn_isolated(
        name: &str,
        command: &str,
        args: &[String],
        env: &[(String, String)],
        worker: Option<&std::path::Path>,
    ) -> Result<Self, String> {
        Self::spawn_stopped(name, command, args, env, worker, &std::sync::atomic::AtomicBool::new(false))
    }
    pub fn spawn_stopped(
        name: &str,
        command: &str,
        args: &[String],
        env: &[(String, String)],
        worker: Option<&std::path::Path>,
        stop: &std::sync::atomic::AtomicBool,
    ) -> Result<Self, String> {
        let mut process = superfik_process::command(worker.unwrap_or_else(|| std::path::Path::new(command)));
        crate::worker::clean_env(&mut process);
        if worker.is_some() {
            process.arg("--mcp");
        } else {
            process.args(args).envs(env.iter().cloned());
        }
        process.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
        let mut tree = Tree::spawn(&mut process).map_err(|error| format!("{name} could not start: {error}"))?;
        let mut input = tree.child().stdin.take().ok_or_else(|| format!("{name} has no input"))?;
        let output = tree.child().stdout.take().ok_or_else(|| format!("{name} has no output"))?;
        let mut output = BufReader::new(output);
        if worker.is_some() {
            let config = serde_json::json!({"command":command,"args":args,"env":env});
            writeln!(input, "{config}").and_then(|_| input.flush()).map_err(|e| e.to_string())?;
            let (ready_tx, ready_rx) = channel();
            std::thread::spawn(move || {
                let mut ready = String::new();
                let result = output.read_line(&mut ready);
                let _ = ready_tx.send((result, ready, output));
            });
            let started = std::time::Instant::now();
            loop {
                if stop.load(std::sync::atomic::Ordering::Relaxed) || started.elapsed() > Duration::from_secs(15) {
                    tree.kill();
                    return Err("MCP worker startup cancelled or timed out".into());
                }
                match ready_rx.recv_timeout(Duration::from_millis(25)) {
                    Ok((result, ready, reader)) => {
                        result.map_err(|e| e.to_string())?;
                        if ready.trim() != "superfik-mcp-ready" {
                            return Err("isolated MCP worker did not become ready".into());
                        }
                        output = reader;
                        break;
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(_) => return Err("MCP worker stopped".into()),
                }
            }
        }
        let (send, lines) = channel();
        std::thread::Builder::new()
            .name(format!("superfik-mcp-{name}"))
            .spawn(move || {
                for line in output.lines().map_while(Result::ok) {
                    if send.send(line).is_err() {
                        return;
                    }
                }
            })
            .map_err(|error| error.to_string())?;
        Ok(Self {
            requests: Mutex::new(()),
            name: name.to_owned(),
            tree: Mutex::new(tree),
            input: Mutex::new(input),
            lines: Mutex::new(lines),
        })
    }

    fn write(&self, message: &Value) -> Result<(), String> {
        let mut input = self.input.lock().map_err(|_| "the server's input is unavailable")?;
        writeln!(input, "{message}").and_then(|()| input.flush()).map_err(|error| format!("{}: {error}", self.name))
    }
}

impl Transport for StdioTransport {
    fn request(&self, message: &Value) -> Result<Value, String> {
        self.request_stopped(message, &std::sync::atomic::AtomicBool::new(false))
    }
    fn request_stopped(&self, message: &Value, stop: &std::sync::atomic::AtomicBool) -> Result<Value, String> {
        let _request = loop {
            if stop.load(std::sync::atomic::Ordering::Relaxed) {
                return Err("MCP call cancelled".into());
            }
            match self.requests.try_lock() {
                Ok(guard) => break guard,
                Err(std::sync::TryLockError::WouldBlock) => std::thread::sleep(Duration::from_millis(25)),
                Err(_) => return Err("MCP request queue unavailable".into()),
            }
        };
        self.write(message)?;
        let deadline = std::time::Instant::now() + ANSWER_WAIT;
        let lines = self.lines.lock().map_err(|_| "the server's output is unavailable")?;
        loop {
            if stop.load(std::sync::atomic::Ordering::Relaxed) || std::time::Instant::now() > deadline {
                self.tree.lock().map_err(|_| "MCP process unavailable")?.kill();
                return Err("MCP call cancelled or timed out".into());
            }
            let line = match lines.recv_timeout(Duration::from_millis(25)) {
                Ok(line) => line,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(_) => return Err(format!("{} stopped", self.name)),
            };
            let Ok(reply) = serde_json::from_str::<Value>(&line) else { continue };
            if let Some(method) = reply.get("method").and_then(Value::as_str) {
                if let Some(id) = reply.get("id") {
                    let answer = if method == "ping" {
                        serde_json::json!({"jsonrpc":"2.0","id":id,"result":{}})
                    } else {
                        serde_json::json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"Client method not supported"}})
                    };
                    self.write(&answer)?;
                }
                continue;
            }
            if reply.get("id") == message.get("id") && reply.get("method").is_none() {
                return Ok(reply);
            }
        }
    }

    fn notify(&self, message: &Value) -> Result<(), String> {
        self.write(message)
    }
}

impl Drop for StdioTransport {
    fn drop(&mut self) {
        if let Ok(tree) = self.tree.get_mut() {
            tree.kill();
        }
    }
}
