use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{ChildStdin, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use platform::Tree;
use serde_json::{Value, json};

use super::{Listener, Transport};

const ANSWER_WAIT: Duration = Duration::from_secs(600);

type Waiting = Arc<Mutex<HashMap<String, Sender<Value>>>>;

pub struct StdioTransport {
    name: String,
    tree: Mutex<Tree>,
    input: Arc<Mutex<ChildStdin>>,
    waiting: Waiting,
    ended: Arc<AtomicBool>,
    shared: bool,
}

impl std::fmt::Debug for StdioTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StdioTransport").field("name", &self.name).finish_non_exhaustive()
    }
}

fn write(input: &Mutex<ChildStdin>, name: &str, message: &Value) -> Result<(), String> {
    let mut input = input.lock().map_err(|_| "the server's input is unavailable")?;
    writeln!(input, "{message}").and_then(|()| input.flush()).map_err(|error| format!("{name}: {error}"))
}

impl StdioTransport {
    pub fn spawn(name: &str, command: &str, args: &[String], env: &[(String, String)]) -> Result<Self, String> {
        Self::spawn_listening(name, command, args, env, None)
    }

    pub fn spawn_listening(
        name: &str,
        command: &str,
        args: &[String],
        env: &[(String, String)],
        listener: Option<Listener>,
    ) -> Result<Self, String> {
        let mut process = platform::command(command);
        process.args(args).envs(env.iter().cloned());
        process.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
        let mut tree = Tree::spawn(&mut process).map_err(|error| format!("{name} could not start: {error}"))?;
        let input = tree.child().stdin.take().ok_or_else(|| format!("{name} has no input"))?;
        let output = tree.child().stdout.take().ok_or_else(|| format!("{name} has no output"))?;
        let transport = Self {
            name: name.to_owned(),
            tree: Mutex::new(tree),
            input: Arc::new(Mutex::new(input)),
            waiting: Arc::default(),
            ended: Arc::default(),
            shared: listener.is_some(),
        };
        let (waiting, ended, input) =
            (Arc::clone(&transport.waiting), Arc::clone(&transport.ended), Arc::clone(&transport.input));
        let server = name.to_owned();
        std::thread::Builder::new()
            .name(format!("{brand_command}-mcp-{name}", brand_command = brand::COMMAND))
            .spawn(move || {
                for line in BufReader::new(output).lines().map_while(Result::ok) {
                    let Ok(message) = serde_json::from_str::<Value>(&line) else { continue };
                    match (message.get("id"), message.get("method").and_then(Value::as_str)) {
                        (Some(id), None) => {
                            let sender = waiting.lock().ok().and_then(|mut waiting| waiting.remove(&id.to_string()));
                            if let Some(sender) = sender {
                                let _ = sender.send(message);
                            }
                        }
                        (Some(id), Some(method)) => {
                            let _ = write(&input, &server, &super::sse::client_answer(id, method));
                        }
                        (None, Some(method)) => {
                            if let Some(listener) = &listener {
                                listener(method, &message["params"]);
                            }
                        }
                        (None, None) => {}
                    }
                }
                ended.store(true, Ordering::Relaxed);
                if let Ok(mut waiting) = waiting.lock() {
                    waiting.clear();
                }
            })
            .map_err(|error| error.to_string())?;
        Ok(transport)
    }
}

impl Transport for StdioTransport {
    fn request(&self, message: &Value) -> Result<Value, String> {
        self.request_stopped(message, &AtomicBool::new(false))
    }

    fn request_stopped(&self, message: &Value, stop: &AtomicBool) -> Result<Value, String> {
        let id = message.get("id").map(Value::to_string).unwrap_or_default();
        let (send, receive) = channel();
        self.waiting.lock().map_err(|_| "MCP request queue unavailable")?.insert(id.clone(), send);
        let forget = || {
            let _ = self.waiting.lock().map(|mut waiting| waiting.remove(&id));
        };
        if let Err(error) = write(&self.input, &self.name, message) {
            forget();
            return Err(error);
        }
        let deadline = std::time::Instant::now() + ANSWER_WAIT;
        loop {
            match receive.recv_timeout(Duration::from_millis(25)) {
                Ok(reply) => return Ok(reply),
                Err(RecvTimeoutError::Disconnected) => return Err(format!("{} stopped", self.name)),
                Err(RecvTimeoutError::Timeout) => {}
            }
            if self.ended.load(Ordering::Relaxed) {
                forget();
                return Err(format!("{} stopped", self.name));
            }
            if stop.load(Ordering::Relaxed) || std::time::Instant::now() > deadline {
                forget();
                if self.shared {
                    let reason = if stop.load(Ordering::Relaxed) { "cancelled" } else { "timed out" };
                    let params = json!({ "requestId": message.get("id"), "reason": reason });
                    let _ = write(
                        &self.input,
                        &self.name,
                        &json!({ "jsonrpc": "2.0", "method": "notifications/cancelled", "params": params }),
                    );
                } else {
                    self.tree.lock().map_err(|_| "MCP process unavailable")?.kill();
                }
                return Err("MCP call cancelled or timed out".into());
            }
        }
    }

    fn notify(&self, message: &Value) -> Result<(), String> {
        write(&self.input, &self.name, message)
    }
}

impl Drop for StdioTransport {
    fn drop(&mut self) {
        if let Ok(tree) = self.tree.get_mut() {
            tree.kill();
        }
    }
}
