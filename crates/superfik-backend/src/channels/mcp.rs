//! The channel tools, served to agents over MCP on a loopback port (the
//! notes tools' shape). Each run gets a bearer token naming its owner,
//! run and the channel accounts it picked, revoked when the run ends. A
//! reply is a draft for a member to approve unless the run may act alone
//! (`ChannelTool::drafts`); either way the owner's clients hear of it.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use superfik_core::channel::{
    CHANNEL_CONVERSATIONS_TOOL, CHANNEL_READ_TOOL, CHANNEL_SEND_TOOL, CHANNELS_SERVER, ChannelBook, ChannelTool,
    Message, Source,
};
use superfik_core::{ConnectionId, Principal, RunId};

use super::{Channels, lock};
use crate::loopback::{self, RpcError};
use crate::notes::mcp::Publish;

const READ_LIMIT: usize = 30;
const LISTED: usize = 20;

#[derive(Clone)]
struct Grant {
    owner: Principal,
    run: RunId,
    project: PathBuf,
    accounts: Vec<(ConnectionId, String)>,
    drafts: bool,
}

#[derive(Clone)]
pub struct ChannelTools {
    url: String,
    grants: Arc<Mutex<HashMap<String, Grant>>>,
    channels: Channels,
    publish: Publish,
}

impl std::fmt::Debug for ChannelTools {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChannelTools").field("url", &self.url).finish_non_exhaustive()
    }
}

impl ChannelTools {
    pub fn new(channels: Channels, publish: Publish) -> Self {
        Self { url: String::new(), grants: Arc::default(), channels, publish }
    }

    pub fn start(channels: Channels, publish: Publish) -> Option<Self> {
        let server = match tiny_http::Server::http("127.0.0.1:0") {
            Ok(server) => server,
            Err(error) => {
                tracing::warn!(%error, "channel tools unavailable");
                return None;
            }
        };
        let port = server.server_addr().to_ip()?.port();
        let tools = Self { url: format!("http://127.0.0.1:{port}/mcp"), ..Self::new(channels, publish) };
        let served = tools.clone();
        std::thread::Builder::new()
            .name("superfik-channels-mcp".into())
            .spawn(move || {
                for request in server.incoming_requests() {
                    served.serve(request);
                }
            })
            .ok()?;
        Some(tools)
    }

    pub fn grant(
        &self,
        owner: &Principal,
        run: RunId,
        project: PathBuf,
        accounts: Vec<(ConnectionId, String)>,
        drafts: bool,
    ) -> ChannelTool {
        let token = uuid::Uuid::new_v4().simple().to_string();
        lock(&self.grants).insert(token.clone(), Grant { owner: owner.clone(), run, project, accounts, drafts });
        ChannelTool { url: self.url.clone(), token, drafts }
    }

    pub fn revoke(&self, token: &str) {
        lock(&self.grants).remove(token);
    }

    pub fn handle(&self, token: &str, message: &Value) -> Result<Option<Value>, u16> {
        let grant = lock(&self.grants).get(token).cloned().ok_or(401_u16)?;
        Ok(loopback::respond(message, CHANNELS_SERVER, definitions, |name, arguments| {
            self.call(&grant, name, arguments)
        }))
    }

    fn call(&self, grant: &Grant, name: &str, arguments: &Value) -> Result<Value, RpcError> {
        let account = match account(grant, arguments) {
            Ok(account) => account,
            Err(problem) => return Ok(loopback::text(problem, true)),
        };
        let book = || self.channels.book(&grant.owner, account);
        let exchange = arguments.get("conversation").and_then(Value::as_str).map(str::trim).unwrap_or_default();
        Ok(match name {
            CHANNEL_CONVERSATIONS_TOOL => loopback::text(listing(&book()), false),
            CHANNEL_READ_TOOL if exchange.is_empty() => loopback::text("Give the conversation.", true),
            CHANNEL_READ_TOOL => {
                let limit = arguments.get("limit").and_then(Value::as_u64).map_or(READ_LIMIT, |n| n as usize);
                let book = book();
                match book.thread(exchange) {
                    Some(_) => loopback::text(transcript(book.recent(exchange, limit.max(1))), false),
                    None => loopback::text(format!("There is no conversation `{exchange}`."), true),
                }
            }
            CHANNEL_SEND_TOOL => {
                let text = arguments.get("text").and_then(Value::as_str).unwrap_or_default().trim();
                let file = arguments.get("file").and_then(Value::as_str).map(str::trim).filter(|f| !f.is_empty());
                if exchange.is_empty() || (text.is_empty() && file.is_none()) {
                    return Ok(loopback::text("Give the conversation and the text, a file, or both.", true));
                }
                let attachment = match file.map(|file| self.channels.attachment(account, &grant.project, file)) {
                    Some(Err(problem)) => return Ok(loopback::text(problem, true)),
                    Some(Ok(attachment)) => Some(attachment),
                    None => None,
                };
                if grant.drafts {
                    let event = self.channels.draft(&grant.owner, account, exchange, text, attachment, grant.run);
                    (self.publish)(&grant.owner, event);
                    loopback::text("Kept as a draft: a member approves it before it goes out.", false)
                } else {
                    match self.channels.send(&grant.owner, account, exchange, text, attachment) {
                        Ok(()) => loopback::text("Queued: it goes out shortly, typed like a person.", false),
                        Err(problem) => loopback::text(problem, true),
                    }
                }
            }
            _ => return Err(loopback::unknown_tool(name)),
        })
    }

    fn serve(&self, mut request: tiny_http::Request) {
        let Some(token) = loopback::bearer(&request) else { return loopback::reply(request, 401, None) };
        if !lock(&self.grants).contains_key(&token) {
            return loopback::reply(request, 401, None);
        }
        let message = match loopback::read_message(&mut request) {
            Ok(message) => message,
            Err(status) => return loopback::reply(request, status, None),
        };
        match self.handle(&token, &message) {
            Ok(Some(answer)) => loopback::reply(request, 200, Some(answer)),
            Ok(None) => loopback::reply(request, 202, None),
            Err(status) => loopback::reply(request, status, None),
        }
    }
}

fn account(grant: &Grant, arguments: &Value) -> Result<ConnectionId, String> {
    let named = arguments.get("account").and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty());
    match (named, grant.accounts.as_slice()) {
        (None, [(only, _)]) => Ok(*only),
        (None, _) => Err(format!("Say which account: {}.", titles(grant))),
        (Some(name), accounts) => accounts
            .iter()
            .find(|(_, title)| title.eq_ignore_ascii_case(name))
            .map(|(id, _)| *id)
            .ok_or_else(|| format!("No account named {name}; there is {}.", titles(grant))),
    }
}

fn titles(grant: &Grant) -> String {
    grant.accounts.iter().map(|(_, title)| title.as_str()).collect::<Vec<_>>().join(", ")
}

fn definitions() -> Vec<Value> {
    let account = json!({ "type": "string", "description": "The account, as the instructions name it; may be left out when there is one." });
    let conversation = json!({ "type": "string", "description": "The conversation: a phone number with country code, or the id listed." });
    vec![
        json!({
            "name": CHANNEL_CONVERSATIONS_TOOL,
            "description": "List the account's recent conversations: id, name, the last message and whether a member took it over.",
            "inputSchema": { "type": "object", "properties": { "account": account } }
        }),
        json!({
            "name": CHANNEL_READ_TOOL,
            "description": "Read a conversation's latest messages, oldest first.",
            "inputSchema": {
                "type": "object",
                "properties": { "account": account, "conversation": conversation, "limit": { "type": "integer", "minimum": 1 } },
                "required": ["conversation"]
            }
        }),
        json!({
            "name": CHANNEL_SEND_TOOL,
            "description": "Send a message in a conversation: text, a file, or a file with the text as its caption.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "account": account,
                    "conversation": conversation,
                    "text": { "type": "string" },
                    "file": { "type": "string", "description": "A file to send: a path in the project, or one this account received (as channel_read lists it). Up to 16 MB." }
                },
                "required": ["conversation"]
            }
        }),
    ]
}

fn listing(book: &ChannelBook) -> String {
    if book.threads.is_empty() {
        return "No conversations yet.".to_owned();
    }
    book.threads
        .iter()
        .take(LISTED)
        .map(|thread| {
            let name = thread.name.as_deref().map_or(String::new(), |name| format!(" ({name})"));
            let last = thread.messages.last().map_or(String::new(), |m| format!(": {}", line(m)));
            let paused = if thread.exchange.paused_until.is_some() { " [a member answered recently]" } else { "" };
            format!("`{}`{name}{paused}{last}", thread.exchange.id)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn transcript(messages: &[Message]) -> String {
    messages.iter().map(line).collect::<Vec<_>>().join("\n")
}

fn line(message: &Message) -> String {
    let who = match message.source {
        Source::Contact => message.sender_name.clone().unwrap_or_else(|| "Contact".to_owned()),
        Source::Agent => "You".to_owned(),
        Source::Member => "A member (from the phone)".to_owned(),
    };
    let file = message.media.as_ref().map_or(String::new(), |m| {
        let kind = format!("{:?}", m.kind).to_lowercase();
        let name = m.filename.as_deref().map_or(String::new(), |name| format!(" {name}"));
        match &m.file {
            Some(path) => format!(" [{kind}{name}, saved at {}]", path.display()),
            None => format!(" [{kind}{name}, not kept]"),
        }
    });
    format!("{who}{file}: {}", message.text)
}

#[cfg(test)]
mod tests;
