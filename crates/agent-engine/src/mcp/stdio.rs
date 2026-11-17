use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{ChildStdin, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use platform::child::{GRACE, Spawn, Stderr, last_lines};
use platform::sync::Locked;
use platform::{Child, Stop};
use serde_json::{Value, json};
use tokio::sync::{mpsc, oneshot};

use super::{ANSWER_WAIT, Listener, MESSAGE_AT_MOST, McpError, Transport};

const SILENT_AFTER_CANCEL: Duration = if cfg!(test) { Duration::from_secs(1) } else { Duration::from_secs(3) };
const STDERR_LINES: usize = 5;

type Waiting = Arc<Mutex<HashMap<String, oneshot::Sender<Value>>>>;

enum Answer {
    Unsent,
    Replied(Option<Value>),
    Ended(&'static str),
}

struct Heard {
    waiting: Waiting,
    ended: AtomicBool,
    messages: AtomicU64,
    problem: Mutex<Option<String>>,
}

pub struct StdioTransport {
    name: String,
    child: Arc<Child>,
    input: Input,
    heard: Arc<Heard>,
    shared: bool,
}

impl std::fmt::Debug for StdioTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StdioTransport").field("name", &self.name).finish_non_exhaustive()
    }
}

type Input = mpsc::Sender<String>;

const QUEUED_AT_MOST: usize = 64;

fn write(input: &Input, name: &str, message: &Value) -> Result<(), String> {
    input.try_send(message.to_string()).map_err(|error| match error {
        mpsc::error::TrySendError::Full(_) => format!("{name} is not reading what it is sent"),
        mpsc::error::TrySendError::Closed(_) => format!("{name}: its input is closed"),
    })
}

fn writer(name: &str, mut stdin: ChildStdin) -> std::io::Result<Input> {
    let (input, mut queued) = mpsc::channel::<String>(QUEUED_AT_MOST);
    std::thread::Builder::new().name(format!("{}-mcp-{name}-input", brand::COMMAND)).spawn(move || {
        while let Some(line) = queued.blocking_recv() {
            if writeln!(stdin, "{line}").and_then(|()| stdin.flush()).is_err() {
                return;
            }
        }
    })?;
    Ok(input)
}

fn line(output: &mut impl BufRead, into: &mut Vec<u8>) -> Result<bool, String> {
    into.clear();
    let most = u64::try_from(MESSAGE_AT_MOST).unwrap_or(u64::MAX).saturating_add(1);
    let read = output.by_ref().take(most).read_until(b'\n', into).map_err(|error| error.to_string())?;
    if into.len() > MESSAGE_AT_MOST && !into.ends_with(b"\n") {
        return Err(format!("sent a line longer than {} MiB", MESSAGE_AT_MOST / (1024 * 1024)));
    }
    Ok(read > 0)
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
        Self::spawn_in(name, command, args, env, &super::StdioSandbox::Unconfined, listener)
    }

    pub fn spawn_in(
        name: &str,
        command: &str,
        args: &[String],
        env: &[(String, String)],
        sandbox: &super::StdioSandbox,
        listener: Option<Listener>,
    ) -> Result<Self, String> {
        let mut process = platform::command(command);
        process.args(args).envs(env.iter().cloned());
        let mut process = sandbox.applied(name, process)?;
        process.stdin(Stdio::piped()).stdout(Stdio::piped());
        let child = Child::spawn_with(&mut process, Spawn { stderr: Stderr::Tail, ..Spawn::default() })
            .map_err(|error| format!("{name} could not start: {error}"))?;
        let input = child.take_stdin().ok_or_else(|| format!("{name} has no input"))?;
        let output = child.take_stdout().ok_or_else(|| format!("{name} has no output"))?;
        let heard = Arc::new(Heard {
            waiting: Arc::default(),
            ended: AtomicBool::new(false),
            messages: AtomicU64::new(0),
            problem: Mutex::new(None),
        });
        let transport = Self {
            name: name.to_owned(),
            child: Arc::new(child),
            input: writer(name, input).map_err(|error| format!("{name}: {error}"))?,
            heard,
            shared: listener.is_some(),
        };
        let reading = Reading {
            server: name.to_owned(),
            input: transport.input.clone(),
            heard: Arc::clone(&transport.heard),
            child: Arc::downgrade(&transport.child),
            listener,
        };
        std::thread::Builder::new()
            .name(format!("{brand_command}-mcp-{name}", brand_command = brand::COMMAND))
            .spawn(move || reading.all(BufReader::new(output)))
            .map_err(|error| error.to_string())?;
        Ok(transport)
    }

    fn stopped(&self) -> String {
        if let Some(problem) = self.heard.problem.locked().clone() {
            return problem;
        }
        let said = last_lines(&self.child.stderr_tail(), STDERR_LINES);
        if said.is_empty() { format!("{} stopped", self.name) } else { format!("{} stopped: {said}", self.name) }
    }

    fn cancel(&self, message: &Value, reason: &str) {
        let params = json!({ "requestId": message.get("id"), "reason": reason });
        let cancelled = json!({ "jsonrpc": "2.0", "method": "notifications/cancelled", "params": params });
        let _ = write(&self.input, &self.name, &cancelled);
        if self.shared {
            return;
        }
        let heard_so_far = self.heard.messages.load(Ordering::Relaxed);
        let (heard, child) = (Arc::clone(&self.heard), Arc::downgrade(&self.child));
        let watching = std::thread::Builder::new().name(format!("{}-mcp-cancelled", brand::COMMAND)).spawn(move || {
            std::thread::sleep(SILENT_AFTER_CANCEL);
            let silent = heard.messages.load(Ordering::Relaxed) == heard_so_far;
            if let Some(child) = child.upgrade().filter(|_| silent) {
                child.terminate(GRACE);
            }
        });
        if watching.is_err() {
            self.child.terminate(GRACE);
        }
    }
}

struct Reading {
    server: String,
    input: Input,
    heard: Arc<Heard>,
    child: Weak<Child>,
    listener: Option<Listener>,
}

impl Reading {
    fn all(self, mut output: impl BufRead) {
        let mut bytes = Vec::new();
        loop {
            match line(&mut output, &mut bytes) {
                Ok(true) => self.route(&bytes),
                Ok(false) => break,
                Err(problem) => {
                    *self.heard.problem.locked() = Some(format!("{} {problem}", self.server));
                    if let Some(child) = self.child.upgrade() {
                        child.terminate(GRACE);
                    }
                    break;
                }
            }
        }
        self.heard.ended.store(true, Ordering::Relaxed);
        self.heard.waiting.locked().clear();
    }

    fn route(&self, line: &[u8]) {
        let Ok(message) = serde_json::from_slice::<Value>(line) else { return };
        self.heard.messages.fetch_add(1, Ordering::Relaxed);
        match (message.get("id"), message.get("method").and_then(Value::as_str)) {
            (Some(id), None) => {
                let sender = self.heard.waiting.locked().remove(&id.to_string());
                if let Some(sender) = sender {
                    let _ = sender.send(message);
                }
            }
            (Some(id), Some(method)) => {
                let _ = write(&self.input, &self.server, &super::sse::client_answer(id, method));
            }
            (None, Some(method)) => {
                if let Some(listener) = &self.listener {
                    listener(method, &message["params"]);
                }
            }
            (None, None) => {}
        }
    }
}

impl Transport for StdioTransport {
    fn request(&self, message: &Value) -> Result<Value, McpError> {
        self.request_stopped(message, &Stop::new())
    }

    fn request_stopped(&self, message: &Value, stop: &Stop) -> Result<Value, McpError> {
        let id = message.get("id").map(Value::to_string).unwrap_or_default();
        let (send, receive) = oneshot::channel();
        self.heard.waiting.locked().insert(id.clone(), send);
        let forget = || {
            self.heard.waiting.locked().remove(&id);
        };
        if self.heard.ended.load(Ordering::Relaxed) {
            forget();
            return Err(self.stopped().into());
        }
        let line = message.to_string();
        let waited = platform::runtime::block_on(async {
            let answered = async {
                if self.input.send(line).await.is_err() {
                    return Answer::Unsent;
                }
                Answer::Replied(receive.await.ok())
            };
            tokio::select! {
                biased;
                answer = answered => answer,
                () = stop.wait() => Answer::Ended("cancelled"),
                () = tokio::time::sleep(ANSWER_WAIT) => Answer::Ended("timed out"),
            }
        })
        .map_err(|error| error.to_string())?;
        match waited {
            Answer::Unsent => {
                forget();
                let closed = format!("{}: its input is closed", self.name);
                Err(if self.child.is_alive() { closed } else { self.stopped() }.into())
            }
            Answer::Replied(Some(reply)) => Ok(reply),
            Answer::Replied(None) => Err(self.stopped().into()),
            Answer::Ended(reason) => {
                forget();
                self.cancel(message, reason);
                Err("MCP call cancelled or timed out".into())
            }
        }
    }

    fn notify(&self, message: &Value) -> Result<(), McpError> {
        write(&self.input, &self.name, message).map_err(McpError::from)
    }

    fn alive(&self) -> bool {
        !self.heard.ended.load(Ordering::Relaxed) && self.child.is_alive()
    }

    fn close(&self) {
        self.child.terminate(GRACE);
    }
}

#[cfg(test)]
#[cfg(unix)]
#[path = "stdio_tests.rs"]
mod tests;
