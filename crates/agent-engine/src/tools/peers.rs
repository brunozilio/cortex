use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, PoisonError};

use domain::Principal;
use domain::inbox::Notice;
use serde_json::{Value, json};

use super::{Access, Outcome, Tool, Workspace};
use crate::model::ToolSpec;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Peer {
    pub client: u64,
    pub name: String,
    pub cwd: PathBuf,
    owner: String,
}

fn lock() -> std::sync::MutexGuard<'static, BTreeMap<u64, Peer>> {
    static ALL: OnceLock<Mutex<BTreeMap<u64, Peer>>> = OnceLock::new();
    ALL.get_or_init(Mutex::default).lock().unwrap_or_else(PoisonError::into_inner)
}

fn owner_key(owner: Option<&Principal>) -> String {
    serde_json::to_string(owner.unwrap_or(&Principal::LocalOwner)).unwrap_or_default()
}

pub fn register(client: u64, owner: Option<&Principal>, cwd: &Path) -> String {
    let mut all = lock();
    if let Some(peer) = all.get_mut(&client) {
        peer.cwd = cwd.to_path_buf();
        peer.owner = owner_key(owner);
        return peer.name.clone();
    }
    let folder = cwd.file_name().map_or_else(|| "session".to_owned(), |name| name.to_string_lossy().into_owned());
    let folder: String = folder.chars().filter(|c| c.is_alphanumeric() || "-_.".contains(*c)).collect();
    let name = format!("{}-{client}", if folder.is_empty() { "session" } else { &folder });
    all.insert(client, Peer { client, name: name.clone(), cwd: cwd.to_path_buf(), owner: owner_key(owner) });
    name
}

pub fn forget(client: u64) {
    lock().remove(&client);
}

pub fn of_owner(owner: Option<&Principal>) -> Vec<Peer> {
    let key = owner_key(owner);
    lock().values().filter(|peer| peer.owner == key).cloned().collect()
}

pub fn send(client: u64, owner: Option<&Principal>, to: &str, message: &str) -> Result<String, String> {
    let sessions = of_owner(owner);
    let from = sessions.iter().find(|peer| peer.client == client).map_or("another session", |peer| &peer.name);
    let target = sessions.iter().find(|peer| peer.name == to).ok_or_else(|| {
        let names: Vec<&str> = sessions.iter().filter(|p| p.client != client).map(|p| p.name.as_str()).collect();
        format!("No session {to}. Open now: {}", if names.is_empty() { "none".into() } else { names.join(", ") })
    })?;
    if target.client == client {
        return Err("That is this session.".into());
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

    fn access(&self) -> Access {
        Access::Executes
    }

    fn summary(&self, input: &Value) -> String {
        format!("to session {}", input["to"].as_str().unwrap_or_default())
    }

    fn run(&self, input: &Value, workspace: &Workspace) -> Outcome {
        let (to, message) = (input["to"].as_str().unwrap_or_default(), input["message"].as_str().unwrap_or_default());
        if message.trim().is_empty() {
            return Outcome::error("Missing required parameter `message`.");
        }
        match send(workspace.credentials.client, workspace.credentials.owner.as_ref(), to.trim(), message) {
            Ok(name) => Outcome::ok(format!("Sent to session {name}; its answer, if any, reaches you as an event.")),
            Err(error) => Outcome::error(error),
        }
    }
}

#[derive(Debug)]
pub struct ListSessions;

impl Tool for ListSessions {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "ListSessions".into(),
            description: crate::tools::describe("ListSessions", &serde_json::Value::Null),
            schema: json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        }
    }

    fn access(&self) -> Access {
        Access::Reads
    }

    fn summary(&self, _input: &Value) -> String {
        String::new()
    }

    fn run(&self, _input: &Value, workspace: &Workspace) -> Outcome {
        let client = workspace.credentials.client;
        let sessions = of_owner(workspace.credentials.owner.as_ref());
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

    #[test]
    fn sessions_message_each_other_within_one_owner() {
        let me = register(9401, None, Path::new("/work/api"));
        let other = register(9402, None, Path::new("/work/web"));
        let stranger =
            Principal::Member(domain::principal::Member { user: domain::UserId::new(), org: domain::OrgId::LOCAL });
        register(9403, Some(&stranger), Path::new("/work/secret"));
        assert_eq!((me.as_str(), other.as_str()), ("api-9401", "web-9402"));
        assert_eq!(register(9401, None, Path::new("/work/api2")), "api-9401", "the name stays");

        assert_eq!(send(9401, None, "web-9402", "is the API up?"), Ok("web-9402".into()));
        assert_eq!(super::super::inbox::take(9402), [Notice::new("session api-9401", "is the API up?")]);
        assert!(send(9401, None, "secret-9403", "hi").is_err(), "another owner's session is out of reach");
        assert!(send(9401, None, "api-9401", "me").is_err());
        assert!(!of_owner(None).iter().any(|peer| peer.client == 9403));
        assert_eq!(SessionMessage.access(), Access::Executes, "another session may run unasked; this asks");
        let mut workspace = Workspace::new("/work/api");
        workspace.credentials.client = 9401;
        let sent = SessionMessage.run(&json!({"to": "web-9402", "message": "deploy?"}), &workspace);
        assert!(!sent.is_error, "{sent:?}");
        assert_eq!(super::super::inbox::take(9402)[0].text, "deploy?");
        for client in [9401, 9402, 9403] {
            forget(client);
        }
    }
}
