use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use platform::sync::Locked;

use domain::inbox::Notice;
use serde_json::Value;

use super::{Outcome, Tool, Workspace};
use crate::model::ToolSpec;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Peer {
    pub client: u64,
    pub name: String,
    pub cwd: PathBuf,
}

fn lock() -> std::sync::MutexGuard<'static, BTreeMap<u64, Peer>> {
    static ALL: OnceLock<Mutex<BTreeMap<u64, Peer>>> = OnceLock::new();
    ALL.get_or_init(Mutex::default).locked()
}

pub fn register(client: u64, cwd: &Path) -> String {
    let mut all = lock();
    if let Some(peer) = all.get_mut(&client) {
        peer.cwd = cwd.to_path_buf();
        return peer.name.clone();
    }
    let folder = cwd.file_name().map_or_else(|| "session".to_owned(), |name| name.to_string_lossy().into_owned());
    let folder: String = folder.chars().filter(|c| c.is_alphanumeric() || "-_.".contains(*c)).collect();
    let name = format!("{}-{client}", if folder.is_empty() { "session" } else { &folder });
    all.insert(client, Peer { client, name: name.clone(), cwd: cwd.to_path_buf() });
    name
}

pub fn forget(client: u64) {
    lock().remove(&client);
}

pub fn open() -> Vec<Peer> {
    lock().values().cloned().collect()
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PeerError {
    #[error("No session {to}. Open now: {}", if open.is_empty() { "none".to_owned() } else { open.join(", ") })]
    NoSuchSession { to: String, open: Vec<String> },
    #[error("That is this session.")]
    ThisSession,
}

pub fn send(client: u64, to: &str, message: &str) -> Result<String, PeerError> {
    let sessions = open();
    let from = sessions.iter().find(|peer| peer.client == client).map_or("another session", |peer| &peer.name);
    let target = sessions.iter().find(|peer| peer.name == to).ok_or_else(|| PeerError::NoSuchSession {
        to: to.to_owned(),
        open: sessions.iter().filter(|p| p.client != client).map(|p| p.name.clone()).collect(),
    })?;
    if target.client == client {
        return Err(PeerError::ThisSession);
    }
    super::inbox::push(target.client, Notice::new(format!("session {from}"), message));
    Ok(target.name.clone())
}

#[derive(Debug)]
pub struct SessionMessage;

impl Tool for SessionMessage {
    fn spec(&self) -> ToolSpec {
        crate::tools::send_message::SendMessageTool.spec()
    }

    fn effects(&self, input: &Value, _workspace: &Workspace) -> Vec<super::Effect> {
        vec![super::Effect::Session { to: input["to"].as_str().unwrap_or_default().trim().to_owned() }]
    }

    fn summary(&self, input: &Value) -> String {
        format!("to session {}", input["to"].as_str().unwrap_or_default())
    }

    fn run(&self, input: &Value, workspace: &Workspace) -> Outcome {
        let (to, message) = (input["to"].as_str().unwrap_or_default(), input["message"].as_str().unwrap_or_default());
        if message.trim().is_empty() {
            return Outcome::error("Missing required parameter `message`.");
        }
        match send(workspace.credentials.client, to.trim(), message) {
            Ok(name) => Outcome::ok(format!("Sent to session {name}; its answer, if any, reaches you as an event.")),
            Err(error) => Outcome::error(error.to_string()),
        }
    }
}

#[derive(Debug)]
pub struct ListSessions;

impl Tool for ListSessions {
    fn spec(&self) -> ToolSpec {
        crate::tools::declared("ListSessions", &Value::Null)
    }

    fn effects(&self, _input: &Value, _workspace: &Workspace) -> Vec<super::Effect> {
        Vec::new()
    }

    fn summary(&self, _input: &Value) -> String {
        String::new()
    }

    fn run(&self, _input: &Value, workspace: &Workspace) -> Outcome {
        let client = workspace.credentials.client;
        let sessions = open();
        let me = sessions.iter().find(|peer| peer.client == client).map_or("unnamed", |peer| peer.name.as_str());
        let others: Vec<String> = sessions
            .iter()
            .filter(|peer| peer.client != client)
            .map(|peer| format!("{} ({})", peer.name, peer.cwd.display()))
            .collect();
        if others.is_empty() {
            return Outcome::ok(format!("This session is {me}; no other session is open."));
        }
        Outcome::ok(format!("This session is {me}. Others:\n{}", others.join("\n")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn open_sessions_message_each_other() {
        let me = register(9401, Path::new("/work/api"));
        let other = register(9402, Path::new("/work/web"));
        assert_eq!((me.as_str(), other.as_str()), ("api-9401", "web-9402"));
        assert_eq!(register(9401, Path::new("/work/api2")), "api-9401", "the name stays");

        assert_eq!(send(9401, "web-9402", "is the API up?"), Ok("web-9402".into()));
        assert_eq!(super::super::inbox::take(9402), [Notice::new("session api-9401", "is the API up?")]);
        let gone = send(9401, "gone-9403", "hi").expect_err("a session that is not open is out of reach");
        assert!(gone.to_string().starts_with("No session gone-9403. Open now: "), "{gone}");
        assert!(gone.to_string().contains("web-9402") && !gone.to_string().contains("api-9401"), "{gone}");
        assert_eq!(send(9401, "api-9401", "me"), Err(PeerError::ThisSession));
        assert_eq!(PeerError::ThisSession.to_string(), "That is this session.");
        let alone = PeerError::NoSuchSession { to: "web-1".into(), open: Vec::new() };
        assert_eq!(alone.to_string(), "No session web-1. Open now: none");
        assert!(open().iter().any(|peer| peer.client == 9402));
        let mut workspace = Workspace::new("/work/api");
        workspace.credentials.client = 9401;
        let effects = SessionMessage.effects(&json!({"to": "web-9402", "message": "deploy?"}), &workspace);
        assert_eq!(
            effects,
            [super::super::Effect::Session { to: "web-9402".into() }],
            "another session may run unasked"
        );
        assert!(effects[0].acts(), "so this asks");
        let sent = SessionMessage.run(&json!({"to": "web-9402", "message": "deploy?"}), &workspace);
        assert!(!sent.is_error, "{sent:?}");
        assert_eq!(super::super::inbox::take(9402)[0].text, "deploy?");
        for client in [9401, 9402] {
            forget(client);
        }
    }
}
