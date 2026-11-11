//! What a person changes: approving a project's server or turning it off
//! (their settings' lists), and adding or removing their own servers
//! (`mcpServers` in their `mcp.json`). Each takes the file's JSON and
//! returns it changed, leaving every other key alone.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use super::{McpApproval, is_free_name};

const ENABLED: &str = "enabledMcpServers";
const DISABLED: &str = "disabledMcpServers";

/// Where a server a run can use was configured.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "name", rename_all = "snake_case")]
pub enum McpOrigin {
    Host,
    Project,
    User,
    Plugin(String),
}

/// Whether a server starts with the next run, and why not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpState {
    Ready,
    /// A project's server nobody approved yet.
    Pending,
    /// A project's server the owner turned off.
    Off,
}

impl McpState {
    pub fn of(origin: &McpOrigin, name: &str, approval: &McpApproval) -> Self {
        if *origin != McpOrigin::Project || approval.allows(name) {
            Self::Ready
        } else if approval.disabled.iter().any(|known| known == name) {
            Self::Off
        } else {
            Self::Pending
        }
    }
}

/// A server as `{command} mcp list` shows it: never its headers or env.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpServerInfo {
    pub name: String,
    pub origin: McpOrigin,
    /// The URL of an HTTP server, the command line of a stdio one.
    pub target: String,
    pub state: McpState,
}

/// Approves a project's server (`true`) or turns it off (`false`).
pub fn set_approval(settings: Value, name: &str, approve: bool) -> Value {
    let mut settings = into_object(settings);
    let mut list = |key: &str, keep: bool| {
        let mut names: Vec<Value> = settings
            .get(key)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|n| n != name)
            .collect();
        if keep {
            names.push(json!(name));
        }
        settings.insert(key.to_owned(), Value::Array(names));
    };
    list(ENABLED, approve);
    list(DISABLED, !approve);
    Value::Object(settings)
}

/// A server as a person types it: a URL is an HTTP server (with headers),
/// anything else a command (with arguments and environment).
pub fn server_from(target: &str, args: &[String], env: &[(String, String)], headers: &[(String, String)]) -> Value {
    let pairs = |pairs: &[(String, String)]| -> Value {
        Value::Object(pairs.iter().map(|(key, value)| (key.clone(), json!(value))).collect())
    };
    let mut server = Map::new();
    if target.starts_with("https://") || target.starts_with("http://") {
        server.insert("type".into(), json!("http"));
        server.insert("url".into(), json!(target));
        if !headers.is_empty() {
            server.insert("headers".into(), pairs(headers));
        }
    } else {
        server.insert("command".into(), json!(target));
        if !args.is_empty() {
            server.insert("args".into(), json!(args));
        }
        if !env.is_empty() {
            server.insert("env".into(), pairs(env));
        }
    }
    Value::Object(server)
}

/// Adds or replaces a person's own server.
pub fn add_server(config: Value, name: &str, server: Value) -> Result<Value, String> {
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || "-_".contains(c)) {
        return Err(format!("`{name}` is not a server name: use letters, digits, - and _"));
    }
    if !is_free_name(name) {
        return Err(format!("`{name}` starts with a name {brand_name} keeps for itself", brand_name = brand::NAME));
    }
    let valid = match server["type"].as_str() {
        Some("http" | "sse") => server["url"].is_string(),
        Some("stdio") | None => server["command"].is_string(),
        Some(_) => false,
    };
    if !valid {
        return Err("a server needs a `command` (stdio) or `type: http` (or `sse`) with a `url`".into());
    }
    let mut config = into_object(config);
    let servers = config.entry("mcpServers").or_insert_with(|| json!({}));
    if !servers.is_object() {
        *servers = json!({});
    }
    if let Some(servers) = servers.as_object_mut() {
        servers.insert(name.to_owned(), server);
    }
    Ok(Value::Object(config))
}

/// Removes a person's own server; says whether it was there.
pub fn remove_server(config: Value, name: &str) -> (Value, bool) {
    let mut config = into_object(config);
    let removed = config.get_mut("mcpServers").and_then(Value::as_object_mut).and_then(|s| s.remove(name)).is_some();
    (Value::Object(config), removed)
}

fn into_object(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        _ => Map::new(),
    }
}
