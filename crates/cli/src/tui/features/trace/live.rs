use std::collections::BTreeMap;
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

use platform::sync::Locked;

use super::http::{self, Request};

const NOT_FOUND: &str = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
const JSON: &str = "application/json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prompt {
    pub session: String,
    pub text: String,
}

#[derive(Default)]
struct Shared {
    page: String,
    latest: BTreeMap<String, String>,
    clients: Vec<TcpStream>,
    sessions: String,
    prompts: Vec<Prompt>,
    watched: Option<String>,
}

pub struct Live {
    pub url: String,
    token: String,
    shared: Arc<Mutex<Shared>>,
}

fn token() -> String {
    let half = || {
        let mut hasher = RandomState::new().build_hasher();
        hasher
            .write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos()));
        hasher.finish()
    };
    format!("{:016x}{:016x}", half(), half())
}

struct Server {
    token: String,
    origin: String,
}

impl Live {
    pub fn start() -> std::io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let token = token();
        let origin = format!("http://127.0.0.1:{}", listener.local_addr()?.port());
        let url = format!("{origin}/?token={token}");
        let shared = Arc::new(Mutex::new(Shared::default()));
        let serving = Arc::clone(&shared);
        let server = Arc::new(Server { token: token.clone(), origin });
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let (shared, server) = (Arc::clone(&serving), Arc::clone(&server));
                std::thread::spawn(move || answer(stream, &server, &shared));
            }
        });
        Ok(Self { url, token, shared })
    }

    pub fn events(&self) -> String {
        format!("/events?token={}", self.token)
    }

    pub fn show(&self, page: String) {
        self.shared.locked().page = page;
    }

    pub fn send(&self, session: &str, data: &str) {
        let mut shared = self.shared.locked();
        let message = format!("data: {}\n\n", data.replace('\n', " "));
        shared.clients.retain_mut(|client| client.write_all(message.as_bytes()).and_then(|()| client.flush()).is_ok());
        shared.latest.insert(session.to_owned(), message);
    }

    pub fn list(&self, sessions: String) {
        self.shared.locked().sessions = sessions;
    }

    pub fn take_prompts(&self) -> Vec<Prompt> {
        std::mem::take(&mut self.shared.locked().prompts)
    }

    pub fn watched(&self) -> Option<String> {
        self.shared.locked().watched.clone()
    }
}

fn answer(mut stream: TcpStream, server: &Server, shared: &Mutex<Shared>) {
    let request = http::read(&stream).filter(|request| request.token.as_deref() == Some(server.token.as_str()));
    let Some(request) = request else {
        let _ = stream.write_all(NOT_FOUND.as_bytes());
        return;
    };
    if request.path == "/events" && request.method == "GET" {
        return listen(stream, shared);
    }
    let reply = routed(&request, server, shared).unwrap_or_else(|| NOT_FOUND.to_owned());
    let _ = stream.write_all(reply.as_bytes());
}

fn routed(request: &Request, server: &Server, shared: &Mutex<Shared>) -> Option<String> {
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/") => Some(http::reply("200 OK", "text/html; charset=utf-8", &shared.locked().page)),
        ("GET", "/sessions") => {
            let sessions = shared.locked().sessions.clone();
            Some(http::reply("200 OK", JSON, if sessions.is_empty() { "[]" } else { &sessions }))
        }
        ("POST", path) if request.origin.as_ref().is_none_or(|origin| *origin == server.origin) => {
            changed(path, &request.body, shared).map(|()| http::reply("202 Accepted", JSON, "{}"))
        }
        _ => None,
    }
}

fn changed(path: &str, body: &[u8], shared: &Mutex<Shared>) -> Option<()> {
    match path {
        "/prompt" => {
            let (session, text) = (named(body, "session")?, named(body, "text")?);
            let text = text.trim();
            (!text.is_empty()).then_some(())?;
            shared.locked().prompts.push(Prompt { session, text: text.to_owned() });
        }
        "/watch" => {
            shared.locked().watched = Some(named(body, "session")?);
        }
        _ => return None,
    }
    Some(())
}

fn named(body: &[u8], field: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_slice(body).ok()?;
    value[field].as_str().filter(|text| !text.is_empty()).map(str::to_owned)
}

fn listen(mut stream: TcpStream, shared: &Mutex<Shared>) {
    let head = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\nConnection: keep-alive\r\n\r\n";
    let mut shared = shared.locked();
    let latest: String = shared.latest.values().map(String::as_str).collect();
    let opened = stream.write_all(head.as_bytes()).and_then(|()| stream.write_all(latest.as_bytes()));
    if opened.is_ok() && stream.flush().is_ok() {
        shared.clients.push(stream);
    }
}

#[cfg(test)]
#[path = "live_tests.rs"]
mod tests;
