use std::collections::HashMap;
use std::io::{BufReader, Read, Write};
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use crossbeam_channel::{Receiver, RecvTimeoutError, SendTimeoutError, Sender, bounded, unbounded};
use std::time::{Duration, Instant};

use domain::lsp::answer::RequestError;
use domain::lsp::{ServerSpec, uri::uri_of};
use platform::Child;
use platform::child::{Spawn, Stderr, last_lines};
use serde_json::{Value, json};

use super::documents::{Documents, Published};
use super::framing::{read_message, write_message};

const STARTUP: Duration = Duration::from_secs(60);
const TICK: Duration = Duration::from_millis(25);
const SHUTDOWN_WAIT: Duration = Duration::from_secs(2);
const EXIT_WAIT: Duration = Duration::from_secs(1);
const STDERR_LINES: usize = 5;

type Answer = Result<Value, RequestError>;

struct Shared {
    name: String,
    input: Sender<Vec<u8>>,
    pending: Mutex<HashMap<u64, Sender<Answer>>>,
    documents: Mutex<Documents>,
    published: Mutex<Published>,
    arrived: Condvar,
    alive: Arc<AtomicBool>,
}

const QUEUED_AT_MOST: usize = 64;
const NOT_READING_AFTER: Duration = Duration::from_secs(10);

fn writing(
    name: &str,
    mut writer: impl Write + Send + 'static,
    alive: Arc<AtomicBool>,
) -> Result<Sender<Vec<u8>>, String> {
    let (input, queued) = bounded::<Vec<u8>>(QUEUED_AT_MOST);
    std::thread::Builder::new()
        .name(format!("{}-lsp-{name}-input", brand::COMMAND))
        .spawn(move || {
            for message in queued {
                if writer.write_all(&message).and_then(|()| writer.flush()).is_err() {
                    alive.store(false, Ordering::Relaxed);
                    return;
                }
            }
        })
        .map_err(|error| error.to_string())?;
    Ok(input)
}

pub struct LspClient {
    shared: Arc<Shared>,
    next: AtomicU64,
    child: Option<Child>,
}

impl std::fmt::Debug for LspClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LspClient").field("name", &self.shared.name).finish_non_exhaustive()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StartError {
    #[error("{server} could not start: {source}")]
    Spawn {
        server: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{server} has no {pipe}")]
    NoPipe { server: String, pipe: &'static str },
    #[error("{0}")]
    Handshake(String),
}

impl From<StartError> for String {
    fn from(error: StartError) -> Self {
        error.to_string()
    }
}

impl LspClient {
    pub fn spawn(spec: &ServerSpec, program: &Path, root: &Path, stop: &AtomicBool) -> Result<Self, StartError> {
        let mut command = platform::command(program);
        command.args(&spec.args).envs(&spec.env).current_dir(root);
        command.stdin(Stdio::piped()).stdout(Stdio::piped());
        let server = || spec.name.clone();
        let child = Child::spawn_with(&mut command, Spawn { stderr: Stderr::Tail, ..Spawn::default() })
            .map_err(|source| StartError::Spawn { server: server(), source })?;
        let input = child.take_stdin().ok_or_else(|| StartError::NoPipe { server: server(), pipe: "input" })?;
        let output = child.take_stdout().ok_or_else(|| StartError::NoPipe { server: server(), pipe: "output" })?;
        Self::connect(&spec.name, output, input, root, Some(child), stop).map_err(StartError::Handshake)
    }

    pub fn connect(
        name: &str,
        reader: impl Read + Send + 'static,
        writer: impl Write + Send + 'static,
        root: &Path,
        child: Option<Child>,
        stop: &AtomicBool,
    ) -> Result<Self, String> {
        let alive = Arc::new(AtomicBool::new(true));
        let shared = Arc::new(Shared {
            name: name.to_owned(),
            input: writing(name, writer, Arc::clone(&alive))?,
            pending: Mutex::default(),
            documents: Mutex::default(),
            published: Mutex::default(),
            arrived: Condvar::new(),
            alive,
        });
        let reading = Arc::clone(&shared);
        std::thread::Builder::new()
            .name(format!("{brand_command}-lsp-{name}", brand_command = brand::COMMAND))
            .spawn(move || reading.read_all(BufReader::new(reader)))
            .map_err(|error| error.to_string())?;
        let client = Self { shared, next: AtomicU64::new(1), child };
        let root_uri = uri_of(root);
        let folder = root.file_name().and_then(|name| name.to_str()).unwrap_or("root");
        let params = json!({
            "processId": std::process::id(),
            "rootUri": root_uri,
            "workspaceFolders": [{"uri": root_uri, "name": folder}],
            "capabilities": {
                "textDocument": {
                    "synchronization": {"didSave": true},
                    "publishDiagnostics": {"versionSupport": true},
                    "documentSymbol": {"hierarchicalDocumentSymbolSupport": true},
                    "hover": {"contentFormat": ["markdown", "plaintext"]},
                    "definition": {"linkSupport": true},
                    "implementation": {"linkSupport": true},
                    "references": {},
                    "callHierarchy": {}
                },
                "workspace": {"symbol": {}, "configuration": true, "workspaceFolders": true},
                "window": {"workDoneProgress": true}
            }
        });
        client.request("initialize", params, stop, STARTUP)?;
        client.notify("initialized", json!({}))?;
        Ok(client)
    }

    pub fn request(
        &self,
        method: &str,
        params: Value,
        stop: &AtomicBool,
        wait: Duration,
    ) -> Result<Value, RequestError> {
        let shared = &self.shared;
        if !self.is_alive() {
            return Err(RequestError::Failed(self.stopped()));
        }
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (send, answer) = unbounded();
        shared
            .pending
            .lock()
            .map_err(|_| RequestError::Failed("the server's requests are unavailable".into()))?
            .insert(id, send);
        let mut message = json!({"jsonrpc": "2.0", "id": id, "method": method});
        if !params.is_null() {
            message["params"] = params;
        }
        let sent = shared.send(&message).map_err(|_| self.stopped());
        let answered = sent.map_err(RequestError::Failed).and_then(|()| self.wait(&answer, stop, wait));
        if let Ok(mut pending) = shared.pending.lock() {
            pending.remove(&id);
        }
        answered
    }

    fn wait(&self, answer: &Receiver<Answer>, stop: &AtomicBool, wait: Duration) -> Result<Value, RequestError> {
        let name = &self.shared.name;
        let deadline = Instant::now() + wait;
        loop {
            match answer.recv_timeout(TICK) {
                Ok(answer) => return answer,
                Err(RecvTimeoutError::Disconnected) => return Err(RequestError::Failed(self.stopped())),
                Err(RecvTimeoutError::Timeout) if !self.is_alive() => {
                    return Err(RequestError::Failed(self.stopped()));
                }
                Err(RecvTimeoutError::Timeout) if stop.load(Ordering::Relaxed) => {
                    return Err(RequestError::Failed("cancelled".into()));
                }
                Err(RecvTimeoutError::Timeout) if Instant::now() > deadline => {
                    return Err(RequestError::Busy(format!("{name} did not answer in {}s", wait.as_secs())));
                }
                Err(RecvTimeoutError::Timeout) => {}
            }
        }
    }

    fn stopped(&self) -> String {
        let name = &self.shared.name;
        let said = self.child.as_ref().map(|child| last_lines(&child.stderr_tail(), STDERR_LINES)).unwrap_or_default();
        if said.is_empty() { format!("{name} stopped") } else { format!("{name} stopped: {said}") }
    }

    fn notify(&self, method: &str, params: Value) -> Result<(), String> {
        let mut message = json!({"jsonrpc": "2.0", "method": method});
        if !params.is_null() {
            message["params"] = params;
        }
        self.shared.send(&message)
    }

    pub fn sync(&self, file: &Path, language: &str, text: &str) -> Result<i64, String> {
        let uri = uri_of(file);
        let mut documents = self.shared.documents.lock().map_err(|_| "the open files are unavailable")?;
        if let Ok(mut published) = self.shared.published.lock() {
            published.forget(&uri);
        }
        let version = match documents.version(&uri) {
            Some(version) => {
                let version = version + 1;
                let document = json!({"uri": uri, "version": version});
                self.notify(
                    "textDocument/didChange",
                    json!({"textDocument": document, "contentChanges": [{"text": text}]}),
                )?;
                version
            }
            None => {
                while let Some(stalest) = documents.make_room() {
                    self.close(&stalest)?;
                }
                let document = json!({"uri": uri, "languageId": language, "version": 1, "text": text});
                self.notify("textDocument/didOpen", json!({"textDocument": document}))?;
                1
            }
        };
        documents.keep(uri, version);
        Ok(version)
    }

    fn close(&self, uri: &str) -> Result<(), String> {
        if let Ok(mut published) = self.shared.published.lock() {
            published.forget(uri);
        }
        self.notify("textDocument/didClose", json!({"textDocument": {"uri": uri}}))
    }

    pub fn diagnostics(&self, file: &Path, version: i64, wait: Duration) -> Option<Vec<Value>> {
        let uri = uri_of(file);
        let deadline = Instant::now() + wait;
        let mut published = self.shared.published.lock().ok()?;
        loop {
            if let Some(found) = published.get(&uri).filter(|found| found.version.is_none_or(|at| at >= version)) {
                return Some(found.items.clone());
            }
            let left = deadline.checked_duration_since(Instant::now())?;
            if !self.is_alive() {
                return None;
            }
            published = self.shared.arrived.wait_timeout(published, left.min(TICK * 4)).ok()?.0;
        }
    }

    pub fn published(&self) -> Vec<(String, Vec<Value>)> {
        self.shared.published.lock().map(|published| published.all()).unwrap_or_default()
    }

    pub fn is_alive(&self) -> bool {
        self.shared.alive.load(Ordering::Relaxed)
    }
}

impl Shared {
    fn send(&self, message: &Value) -> Result<(), String> {
        if !self.alive.load(Ordering::Relaxed) {
            return Err(format!("{} stopped", self.name));
        }
        let mut framed = Vec::new();
        write_message(&mut framed, message).map_err(|error| format!("{}: {error}", self.name))?;
        self.input.send_timeout(framed, NOT_READING_AFTER).map_err(|error| {
            self.alive.store(false, Ordering::Relaxed);
            match error {
                SendTimeoutError::Timeout(_) => format!("{} stopped reading what it is sent", self.name),
                SendTimeoutError::Disconnected(_) => format!("{} stopped", self.name),
            }
        })
    }

    fn read_all(&self, mut reader: impl std::io::BufRead) {
        while let Ok(Some(message)) = read_message(&mut reader) {
            self.route(message);
        }
        self.alive.store(false, Ordering::Relaxed);
        if let Ok(mut pending) = self.pending.lock() {
            pending.clear();
        }
        self.arrived.notify_all();
    }

    fn route(&self, message: Value) {
        let method = message["method"].as_str();
        match (method, message.get("id")) {
            (Some(method), Some(id)) => {
                let result = match method {
                    "workspace/configuration" => {
                        let asked = message["params"]["items"].as_array().map_or(0, Vec::len);
                        Value::Array(vec![Value::Null; asked])
                    }
                    _ => Value::Null,
                };
                let _ = self.send(&json!({"jsonrpc": "2.0", "id": id, "result": result}));
            }
            (Some("textDocument/publishDiagnostics"), None) => self.publish(&message["params"]),
            (Some(_), None) => {}
            (None, Some(id)) => {
                let Some(id) = id.as_u64() else { return };
                let answer = match message.get("error") {
                    Some(error) => Err(RequestError::answered(
                        &self.name,
                        error["code"].as_i64(),
                        error["message"].as_str().unwrap_or("failed"),
                    )),
                    None => Ok(message.get("result").cloned().unwrap_or(Value::Null)),
                };
                let waiting = self.pending.lock().ok().and_then(|mut pending| pending.remove(&id));
                if let Some(waiting) = waiting {
                    let _ = waiting.send(answer);
                }
            }
            (None, None) => {}
        }
    }

    fn publish(&self, params: &Value) {
        let Some(uri) = params["uri"].as_str() else { return };
        let items = params["diagnostics"].as_array().cloned().unwrap_or_default();
        let over = self.published.lock().is_ok_and(|mut published| {
            published.keep(uri.to_owned(), params["version"].as_i64(), items);
            published.is_over()
        });
        if over {
            let open = self.documents.lock().map(|documents| documents.uris()).unwrap_or_default();
            if let Ok(mut published) = self.published.lock() {
                published.trim(&open);
            }
        }
        self.arrived.notify_all();
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        if !self.is_alive() {
            return;
        }
        let _ = self.request("shutdown", Value::Null, &AtomicBool::new(false), SHUTDOWN_WAIT);
        let _ = self.notify("exit", Value::Null);
        if let Some(child) = &self.child {
            child.wait_until(Instant::now() + EXIT_WAIT);
        }
    }
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
