use std::collections::HashMap;
use std::io::{BufReader, Read, Write};
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use domain::lsp::{ServerSpec, uri::uri_of};
use platform::Tree;
use serde_json::{Value, json};

use super::framing::{read_message, write_message};

const STARTUP: Duration = Duration::from_secs(60);
const TICK: Duration = Duration::from_millis(25);

type Answer = Result<Value, String>;

struct Published {
    version: Option<i64>,
    items: Vec<Value>,
}

struct Shared {
    name: String,
    writer: Mutex<Box<dyn Write + Send>>,
    pending: Mutex<HashMap<u64, Sender<Answer>>>,
    published: Mutex<HashMap<String, Published>>,
    arrived: Condvar,
    alive: AtomicBool,
}

pub struct LspClient {
    shared: Arc<Shared>,
    next: AtomicU64,
    documents: Mutex<HashMap<String, i64>>,
    tree: Option<Mutex<Tree>>,
}

impl std::fmt::Debug for LspClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LspClient").field("name", &self.shared.name).finish_non_exhaustive()
    }
}

impl LspClient {
    pub fn spawn(spec: &ServerSpec, program: &Path, root: &Path, stop: &AtomicBool) -> Result<Self, String> {
        let mut command = platform::command(program);
        command.args(&spec.args).envs(&spec.env).current_dir(root);
        command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
        let mut tree = Tree::spawn(&mut command).map_err(|error| format!("{} could not start: {error}", spec.name))?;
        let input = tree.child().stdin.take().ok_or_else(|| format!("{} has no input", spec.name))?;
        let output = tree.child().stdout.take().ok_or_else(|| format!("{} has no output", spec.name))?;
        Self::connect(&spec.name, output, input, root, Some(tree), stop)
    }

    pub fn connect(
        name: &str,
        reader: impl Read + Send + 'static,
        writer: impl Write + Send + 'static,
        root: &Path,
        tree: Option<Tree>,
        stop: &AtomicBool,
    ) -> Result<Self, String> {
        let shared = Arc::new(Shared {
            name: name.to_owned(),
            writer: Mutex::new(Box::new(writer)),
            pending: Mutex::default(),
            published: Mutex::default(),
            arrived: Condvar::new(),
            alive: AtomicBool::new(true),
        });
        let reading = Arc::clone(&shared);
        std::thread::Builder::new()
            .name(format!("{brand_command}-lsp-{name}", brand_command = brand::COMMAND))
            .spawn(move || reading.read_all(BufReader::new(reader)))
            .map_err(|error| error.to_string())?;
        let client = Self { shared, next: AtomicU64::new(1), documents: Mutex::default(), tree: tree.map(Mutex::new) };
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

    pub fn request(&self, method: &str, params: Value, stop: &AtomicBool, wait: Duration) -> Result<Value, String> {
        let shared = &self.shared;
        if !self.is_alive() {
            return Err(format!("{} stopped", shared.name));
        }
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (send, answer) = channel();
        shared.pending.lock().map_err(|_| "the server's requests are unavailable")?.insert(id, send);
        let sent = shared.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        let answered = sent.and_then(|()| self.wait(&answer, stop, wait));
        if let Ok(mut pending) = shared.pending.lock() {
            pending.remove(&id);
        }
        answered
    }

    fn wait(&self, answer: &Receiver<Answer>, stop: &AtomicBool, wait: Duration) -> Result<Value, String> {
        let name = &self.shared.name;
        let deadline = Instant::now() + wait;
        loop {
            match answer.recv_timeout(TICK) {
                Ok(answer) => return answer.map_err(|error| format!("{name}: {error}")),
                Err(RecvTimeoutError::Disconnected) => return Err(format!("{name} stopped")),
                Err(RecvTimeoutError::Timeout) if !self.is_alive() => return Err(format!("{name} stopped")),
                Err(RecvTimeoutError::Timeout) if stop.load(Ordering::Relaxed) => return Err("cancelled".into()),
                Err(RecvTimeoutError::Timeout) if Instant::now() > deadline => {
                    return Err(format!("{name} did not answer in {}s", wait.as_secs()));
                }
                Err(RecvTimeoutError::Timeout) => {}
            }
        }
    }

    fn notify(&self, method: &str, params: Value) -> Result<(), String> {
        self.shared.send(&json!({"jsonrpc": "2.0", "method": method, "params": params}))
    }

    pub fn sync(&self, file: &Path, language: &str, text: &str) -> Result<i64, String> {
        let uri = uri_of(file);
        let mut documents = self.documents.lock().map_err(|_| "the open files are unavailable")?;
        if let Ok(mut published) = self.shared.published.lock() {
            published.remove(&uri);
        }
        let version = match documents.get(&uri) {
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
                let document = json!({"uri": uri, "languageId": language, "version": 1, "text": text});
                self.notify("textDocument/didOpen", json!({"textDocument": document}))?;
                1
            }
        };
        documents.insert(uri, version);
        Ok(version)
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

    pub fn is_alive(&self) -> bool {
        self.shared.alive.load(Ordering::Relaxed)
    }
}

impl Shared {
    fn send(&self, message: &Value) -> Result<(), String> {
        let mut writer = self.writer.lock().map_err(|_| "the server's input is unavailable")?;
        write_message(&mut *writer, message).map_err(|_| {
            self.alive.store(false, Ordering::Relaxed);
            format!("{} stopped", self.name)
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
                    Some(error) => Err(error["message"].as_str().unwrap_or("failed").to_owned()),
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
        if let Ok(mut published) = self.published.lock() {
            published.insert(uri.to_owned(), Published { version: params["version"].as_i64(), items });
        }
        self.arrived.notify_all();
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        if let Some(tree) = &mut self.tree
            && let Ok(tree) = tree.get_mut()
        {
            tree.kill();
        }
    }
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
