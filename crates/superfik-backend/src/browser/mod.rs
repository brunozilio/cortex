//! The browser card for agents. Every run gets the browser tools over MCP
//! on a loopback port, with a bearer token bound to that run and revoked
//! when it ends. A tool call becomes `EngineEvent::Browse` for the client,
//! which acts on the card beside the run's conversation and answers with
//! `AnswerBrowse`; the call waits for that answer.
//!
//! [`BrowserTools::handle`] is the protocol over a token;
//! [`BrowserTools::start`] adds the socket.

pub mod mcp;
mod mission_tools;

use std::collections::HashMap;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use serde_json::Value;
use superfik_core::browser::BROWSER_SERVER;
use superfik_core::{BrowseId, BrowserAnswer, BrowserOp, BrowserTool, EngineEvent, Principal, RunId};

use crate::loopback::{self, RpcError};
use crate::vault::Notify;

const ANSWER_LIMIT: Duration = Duration::from_secs(60);

struct Grant {
    owner: Principal,
    run: RunId,
}

struct Waiting {
    owner: Principal,
    run: RunId,
    reply: Sender<Result<BrowserAnswer, String>>,
}

pub trait GrantedLogins: Send + Sync {
    fn granted_login(&self, owner: &Principal, run: RunId, website: Option<&str>) -> Result<String, String>;
}

impl GrantedLogins for crate::vault::Vault {
    fn granted_login(&self, owner: &Principal, run: RunId, website: Option<&str>) -> Result<String, String> {
        crate::vault::Vault::granted_login(self, owner, run, website)
    }
}

#[derive(Clone)]
pub struct BrowserTools {
    url: String,
    grants: Arc<Mutex<HashMap<String, Grant>>>,
    waiting: Arc<Mutex<HashMap<String, Waiting>>>,
    notify: Notify,
    limit: Duration,
    logins: Option<Arc<dyn GrantedLogins>>,
}

impl std::fmt::Debug for BrowserTools {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrowserTools").field("url", &self.url).finish_non_exhaustive()
    }
}

impl BrowserTools {
    pub fn new(notify: Notify, limit: Duration) -> Self {
        Self { url: String::new(), grants: Arc::default(), waiting: Arc::default(), notify, limit, logins: None }
    }

    pub fn with_logins(mut self, logins: Arc<dyn GrantedLogins>) -> Self {
        self.logins = Some(logins);
        self
    }

    pub fn start(notify: Notify) -> Option<Self> {
        let server = match tiny_http::Server::http("127.0.0.1:0") {
            Ok(server) => server,
            Err(error) => {
                tracing::warn!(%error, "browser tools unavailable");
                return None;
            }
        };
        let port = server.server_addr().to_ip()?.port();
        let tools = Self { url: format!("http://127.0.0.1:{port}/mcp"), ..Self::new(notify, ANSWER_LIMIT) };
        let served = tools.clone();
        std::thread::Builder::new()
            .name("superfik-browser-mcp".into())
            .spawn(move || {
                for request in server.incoming_requests() {
                    let tools = served.clone();
                    std::thread::spawn(move || tools.serve(request));
                }
            })
            .ok()?;
        Some(tools)
    }

    pub fn grant(&self, owner: &Principal, run: RunId) -> BrowserTool {
        let token = uuid::Uuid::new_v4().simple().to_string();
        lock(&self.grants).insert(token.clone(), Grant { owner: owner.clone(), run });
        BrowserTool { url: self.url.clone(), token }
    }

    pub fn revoke(&self, token: &str) {
        if let Some(grant) = lock(&self.grants).remove(token) {
            lock(&self.waiting).retain(|_, w| w.run != grant.run);
        }
    }

    pub fn answer(&self, owner: &Principal, id: &BrowseId, answer: Result<BrowserAnswer, String>) {
        let mut waiting = lock(&self.waiting);
        if waiting.get(&id.0).is_some_and(|w| &w.owner == owner)
            && let Some(w) = waiting.remove(&id.0)
        {
            let _ = w.reply.send(answer);
        }
    }

    pub fn handle(&self, token: &str, message: &Value) -> Result<Option<Value>, u16> {
        let (owner, run) = lock(&self.grants).get(token).map(|g| (g.owner.clone(), g.run)).ok_or(401_u16)?;
        Ok(loopback::respond(message, BROWSER_SERVER, mcp::tools, |name, arguments| {
            self.call(&owner, run, name, arguments)
        }))
    }

    fn call(&self, owner: &Principal, run: RunId, name: &str, arguments: &Value) -> Result<Value, RpcError> {
        let op = match self.op(owner, run, name, arguments) {
            Ok(op) => op,
            Err(_) if !superfik_core::browser::BROWSER_TOOLS.contains(&name) => {
                return Err(loopback::unknown_tool(name));
            }
            Err(error) => return Ok(loopback::text(error, true)),
        };
        let id = BrowseId(uuid::Uuid::new_v4().simple().to_string());
        let limit = if matches!(op, superfik_core::BrowserOp::FillLogin { .. }) {
            self.limit + crate::vault::APPROVAL_LIMIT
        } else {
            self.limit
        };
        let (reply, answer) = mpsc::channel();
        lock(&self.waiting).insert(id.0.clone(), Waiting { owner: owner.clone(), run, reply });
        (self.notify)(run, EngineEvent::Browse { id: id.clone(), op });
        let answer = match answer.recv_timeout(limit) {
            Ok(answer) => answer,
            Err(RecvTimeoutError::Timeout) => {
                lock(&self.waiting).remove(&id.0);
                Err("The browser card did not answer in time.".to_owned())
            }
            Err(RecvTimeoutError::Disconnected) => Err("The run ended.".to_owned()),
        };
        Ok(mcp::result(answer))
    }

    fn op(&self, owner: &Principal, run: RunId, name: &str, arguments: &Value) -> Result<BrowserOp, String> {
        if name != superfik_core::browser::AUTOFILL_TOOL {
            return mcp::op(name, arguments);
        }
        let logins = self.logins.as_ref().ok_or("The Superfik Vault is not available to this run.")?;
        let website = arguments.get("website").and_then(Value::as_str).map(str::trim).filter(|w| !w.is_empty());
        Ok(BrowserOp::FillLogin { item: logins.granted_login(owner, run, website)? })
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

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests_autofill;

#[cfg(test)]
mod tests {
    use serde_json::json;
    use superfik_core::BrowserOp;

    use super::*;

    type Seen = Arc<Mutex<Vec<(RunId, EngineEvent)>>>;

    fn tools(limit: Duration) -> (BrowserTools, Seen) {
        let seen: Seen = Arc::default();
        let sink = seen.clone();
        let notify: Notify = Arc::new(move |run, event| lock(&sink).push((run, event)));
        (BrowserTools::new(notify, limit), seen)
    }

    fn call(name: &str, arguments: Value) -> Value {
        json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": name, "arguments": arguments } })
    }

    #[test]
    fn a_call_travels_to_the_client_and_waits_for_its_answer() {
        let (tools, seen) = tools(Duration::from_secs(5));
        let owner = Principal::LocalOwner;
        let run = RunId::new();
        let grant = tools.grant(&owner, run);
        let answering = tools.clone();
        let seen_by_client = seen.clone();
        let client = std::thread::spawn(move || {
            loop {
                let browse = lock(&seen_by_client).iter().find_map(|(_, e)| match e {
                    EngineEvent::Browse { id, op } => Some((id.clone(), op.clone())),
                    _ => None,
                });
                if let Some((id, op)) = browse {
                    assert_eq!(op, BrowserOp::Navigate { url: "example.com".into() });
                    answering.answer(&Principal::LocalOwner, &id, Ok(BrowserAnswer::text("Example Domain")));
                    return;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        });
        let reply = tools.handle(&grant.token, &call("browser_navigate", json!({ "url": "example.com" })));
        client.join().expect("client");
        let reply = reply.expect("authorized").expect("answer");
        assert_eq!(reply["result"]["content"][0]["text"], "Example Domain");
        assert!(matches!(lock(&seen)[0], (r, _) if r == run));
    }

    #[test]
    fn bad_arguments_never_reach_the_client_and_strangers_are_refused() {
        let (tools, seen) = tools(Duration::from_secs(5));
        let grant = tools.grant(&Principal::LocalOwner, RunId::new());
        let reply = tools.handle(&grant.token, &call("browser_click", json!({}))).expect("ok").expect("answer");
        assert_eq!(reply["result"]["isError"], true);
        assert!(lock(&seen).is_empty());
        assert_eq!(tools.handle("nope", &call("browser_back", json!({}))), Err(401));
        tools.revoke(&grant.token);
        assert_eq!(tools.handle(&grant.token, &call("browser_back", json!({}))), Err(401));
    }

    #[test]
    fn an_unanswered_call_times_out() {
        let (tools, _) = tools(Duration::from_millis(20));
        let grant = tools.grant(&Principal::LocalOwner, RunId::new());
        let reply = tools.handle(&grant.token, &call("browser_snapshot", json!({}))).expect("ok").expect("answer");
        assert_eq!(reply["result"]["isError"], true);
    }

    #[test]
    fn sign_in_allows_time_for_the_users_approval_before_the_page_response() {
        let (tools, seen) = tools(Duration::from_millis(10));
        let grant = tools.grant(&Principal::LocalOwner, RunId::new());
        let answering = tools.clone();
        let client = std::thread::spawn(move || {
            let until = std::time::Instant::now() + Duration::from_secs(2);
            loop {
                let id = lock(&seen).iter().find_map(|(_, e)| match e {
                    EngineEvent::Browse { id, .. } => Some(id.clone()),
                    _ => None,
                });
                if let Some(id) = id {
                    std::thread::sleep(Duration::from_millis(50));
                    answering.answer(&Principal::LocalOwner, &id, Ok(BrowserAnswer::text("filled after approval")));
                    break;
                }
                assert!(std::time::Instant::now() < until, "no sign-in reached the client");
                std::thread::sleep(Duration::from_millis(1));
            }
        });
        let reply = tools
            .handle(&grant.token, &call("browser_fill_login", json!({"item":"fixture"})))
            .expect("owner")
            .expect("answer");
        client.join().expect("client");
        assert_ne!(reply["result"]["isError"], true, "sign-in timed out before the user could approve it: {reply}");
    }
}
