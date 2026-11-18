use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, Weak};

use domain::Decision;
use domain::channel_relay::{PERMISSION_CAPABILITY, Relayed, asks_for_its_own_tool, permission_request, request_id};
use platform::sync::Locked;
use serde_json::Value;

use super::McpServer;
use crate::control::Answerer;

#[derive(Default)]
struct Relays {
    channels: HashMap<u64, Vec<Weak<McpServer>>>,
    pending: HashMap<String, Pending>,
}

struct Pending {
    client: u64,
    tool: String,
    answerer: Answerer,
}

fn relays() -> MutexGuard<'static, Relays> {
    static RELAYS: OnceLock<Mutex<Relays>> = OnceLock::new();
    RELAYS.get_or_init(Mutex::default).locked()
}

pub fn takes_permission_prompts(server: &McpServer) -> bool {
    server.capabilities()["experimental"][PERMISSION_CAPABILITY].is_object()
}

pub(super) fn register(client: u64, server: &Arc<McpServer>) {
    if takes_permission_prompts(server) {
        let mut relays = relays();
        let channels = relays.channels.entry(client).or_default();
        channels.retain(|open| open.upgrade().is_some_and(|open| open.name() != server.name()));
        channels.push(Arc::downgrade(server));
    }
}

#[derive(Debug)]
pub struct Relay {
    request_id: String,
}

impl Drop for Relay {
    fn drop(&mut self) {
        relays().pending.remove(&self.request_id);
    }
}

pub fn ask(client: u64, call: &str, tool: &str, description: &str, input: &Value, answerer: Answerer) -> Option<Relay> {
    let request_id = request_id(call);
    let channels: Vec<Arc<McpServer>> = {
        let mut relays = relays();
        let open = relays.channels.get_mut(&client)?;
        open.retain(|server| server.upgrade().is_some_and(|server| server.alive()));
        let channels: Vec<Arc<McpServer>> = open.iter().filter_map(Weak::upgrade).collect();
        if channels.is_empty() {
            return None;
        }
        if relays.pending.contains_key(&request_id) {
            tracing::warn!(%request_id, "a permission prompt with this id is already out; answer this one in the terminal");
            return None;
        }
        relays.pending.insert(request_id.clone(), Pending { client, tool: tool.to_owned(), answerer });
        channels
    };
    let message = permission_request(&request_id, tool, description, input);
    for channel in channels {
        if let Err(error) = channel.notify(&message) {
            tracing::warn!(%error, channel = channel.name(), "the permission prompt could not be sent to the channel");
        }
    }
    Some(Relay { request_id })
}

pub(super) fn answered(client: u64, server: &str, relayed: Relayed) {
    let Relayed { request_id, allow } = relayed;
    let mut relays = relays();
    let Some(pending) = relays.pending.get(&request_id) else {
        tracing::warn!(%request_id, channel = server, "a channel answered a permission prompt that is not open");
        return;
    };
    if pending.client != client {
        return;
    }
    if allow && asks_for_its_own_tool(server, &pending.tool) {
        tracing::warn!(%request_id, channel = server, "a channel cannot allow a call to its own tool");
        return;
    }
    if let Some(pending) = relays.pending.remove(&request_id) {
        drop(relays);
        pending.answerer.answer(if allow { Decision::Allow } else { Decision::Deny });
    }
}
