//! X (API v2): direct messages and mentions.
//!
//! The account's fields are the app's `api_key` and `api_secret` and the
//! account's `access_token` and `access_token_secret` (OAuth 1.0a user
//! context, from the developer portal's "Keys and tokens"; they do not
//! expire). Linking reads the user (`/2/users/me`). X is polled every
//! `POLL` (mentions since the last one, and the latest DM events), first
//! up to what is there (history, not news), within the Basic tier's
//! limits. An app on the Account Activity API may also point its webhook
//! at `/hooks/channels/<connection>`: the CRC is answered and signed events
//! are taken at once. A DM goes to `/2/dm_conversations/with/<id>/messages`;
//! a thread is answered by replying to the contact's latest post in it.

mod oauth;
mod parse;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub use oauth::Keys;
pub use parse::{DM, POST, activity, dm_events, mentions, rfc3339_ms};
use serde_json::json;
use superfik_core::channel::{ChannelKind, Delivery, Message, Source};

use crate::http::{Http, HttpRequest, Method};
use crate::recent::Recent;
use crate::{Channel, ChannelError, ChannelEvent, Events, HookAnswer, HookMethod, HookRequest, LinkSpec, Opener};

pub const POLL: Duration = Duration::from_secs(120);

pub type Stamp = Arc<dyn Fn() -> (String, u64) + Send + Sync>;

fn system_stamp() -> (String, u64) {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    (superfik_core::RunId::new().to_string().replace('-', ""), now)
}

pub struct XOpener {
    http: Arc<dyn Http>,
    api: String,
    poll: Option<Duration>,
    stamp: Stamp,
}

impl std::fmt::Debug for XOpener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("XOpener").field("api", &self.api).field("poll", &self.poll).finish_non_exhaustive()
    }
}

impl XOpener {
    pub fn new(http: Arc<dyn Http>) -> Self {
        Self { http, api: "https://api.x.com".into(), poll: Some(POLL), stamp: Arc::new(system_stamp) }
    }

    pub fn at(http: Arc<dyn Http>, api: &str, stamp: Stamp) -> Self {
        Self { http, api: api.trim_end_matches('/').to_owned(), poll: None, stamp }
    }

    pub fn link(&self, spec: &LinkSpec, events: Events) -> Result<Arc<XChannel>, ChannelError> {
        let field = |key: &str| {
            spec.credentials
                .get(key)
                .map(str::to_owned)
                .ok_or_else(|| ChannelError::Failed(format!("the account has no {key}")))
        };
        let keys = Keys {
            consumer_key: field("api_key")?,
            consumer_secret: field("api_secret")?,
            token: field("access_token")?,
            token_secret: field("access_token_secret")?,
        };
        let mut channel = XChannel {
            keys,
            me: String::new(),
            http: Arc::clone(&self.http),
            api: self.api.clone(),
            stamp: Arc::clone(&self.stamp),
            events,
            state: Mutex::default(),
            stopped: AtomicBool::new(false),
        };
        let user = channel.call(Method::Get, &format!("{}/2/users/me", self.api), None)?;
        channel.me =
            user["data"]["id"].as_str().ok_or_else(|| ChannelError::Failed("X named no user".into()))?.to_owned();
        let handle = user["data"]["username"].as_str().map_or_else(|| channel.me.clone(), |u| format!("@{u}"));
        (channel.events)(ChannelEvent::Linked { account: handle });
        Ok(Arc::new(channel))
    }
}

impl Opener for XOpener {
    fn kind(&self) -> ChannelKind {
        ChannelKind::X
    }

    fn open(&self, spec: &LinkSpec, events: Events) -> Result<Arc<dyn Channel>, ChannelError> {
        let channel = self.link(spec, events)?;
        if let Some(every) = self.poll {
            let polled = Arc::clone(&channel);
            std::thread::Builder::new()
                .name("superfik-x".into())
                .spawn(move || {
                    while !polled.stopped.load(Ordering::Relaxed) {
                        polled.poll();
                        std::thread::sleep(every);
                    }
                })
                .map_err(|error| ChannelError::Failed(error.to_string()))?;
        }
        Ok(channel)
    }
}

#[derive(Default)]
struct State {
    read: bool,
    since_mention: Option<String>,
    newest_dm: u64,
    latest: HashMap<String, String>,
    sent: Recent,
    reported: Recent,
}

impl std::fmt::Debug for XChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("XChannel").field("me", &self.me).finish_non_exhaustive()
    }
}

pub struct XChannel {
    keys: Keys,
    me: String,
    http: Arc<dyn Http>,
    api: String,
    stamp: Stamp,
    events: Events,
    state: Mutex<State>,
    stopped: AtomicBool,
}

impl XChannel {
    fn call(
        &self,
        method: Method,
        url: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<serde_json::Value, ChannelError> {
        let (nonce, time) = (self.stamp)();
        let verb = if method == Method::Post { "POST" } else { "GET" };
        let header = oauth::header(&self.keys, verb, url, &[], &nonce, time);
        let request = match body {
            Some(body) => HttpRequest::json(url, body),
            None => HttpRequest::get(url),
        };
        self.http.call(request.header("Authorization", header))?.ok()
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub fn poll(&self) {
        let (first, since) = {
            let state = self.state();
            (!state.read, state.since_mention.clone())
        };
        let delivery = if first { Delivery::Backlog } else { Delivery::Live };
        let mut url = format!(
            "{}/2/users/{}/mentions?tweet.fields=created_at,conversation_id,author_id&expansions=author_id\
             &user.fields=name,username&max_results=20",
            self.api, self.me
        );
        if let Some(since) = &since {
            url.push_str(&format!("&since_id={since}"));
        }
        match self.call(Method::Get, &url, None) {
            Ok(body) => {
                if let Some(newest) = body["meta"]["newest_id"].as_str() {
                    self.state().since_mention = Some(newest.to_owned());
                }
                let mut found = mentions(&body, &self.me);
                found.reverse();
                self.report(found, delivery);
            }
            Err(error) => tracing::warn!(%error, "could not read X mentions"),
        }
        let url = format!(
            "{}/2/dm_events?dm_event.fields=id,text,event_type,created_at,sender_id,dm_conversation_id\
             &expansions=sender_id&user.fields=name,username&max_results=50",
            self.api
        );
        match self.call(Method::Get, &url, None) {
            Ok(body) => {
                let (newest, sent) = {
                    let state = self.state();
                    (state.newest_dm, state.sent.clone())
                };
                let id = |m: &Message| m.id.parse::<u64>().unwrap_or_default();
                let mut found: Vec<Message> =
                    dm_events(&body, &self.me, |id| sent.contains(id)).into_iter().filter(|m| id(m) > newest).collect();
                found.reverse();
                if let Some(top) = found.iter().map(id).max() {
                    self.state().newest_dm = top;
                }
                self.report(found, delivery);
            }
            Err(error) => tracing::warn!(%error, "could not read X direct messages"),
        }
        self.state().read = true;
    }

    fn report(&self, messages: Vec<Message>, delivery: Delivery) {
        for message in messages {
            {
                let mut state = self.state();
                if !state.reported.insert(message.id.clone()) {
                    continue;
                }
                if message.source == Source::Contact && message.exchange.starts_with(POST) {
                    state.latest.insert(message.exchange.clone(), message.id.clone());
                }
            }
            (self.events)(ChannelEvent::Message { message, delivery });
        }
    }
}

impl Channel for XChannel {
    fn send_text(&self, exchange: &str, text: &str) -> Result<String, ChannelError> {
        let id = if let Some(user) = exchange.strip_prefix(DM) {
            let url = format!("{}/2/dm_conversations/with/{user}/messages", self.api);
            let sent = self.call(Method::Post, &url, Some(&json!({ "text": text })))?;
            sent["data"]["dm_event_id"].as_str().unwrap_or_default().to_owned()
        } else if let Some(thread) = exchange.strip_prefix(POST) {
            let to = self.state().latest.get(exchange).cloned().unwrap_or_else(|| thread.to_owned());
            let body = json!({ "text": text, "reply": { "in_reply_to_tweet_id": to } });
            let sent = self.call(Method::Post, &format!("{}/2/tweets", self.api), Some(&body))?;
            sent["data"]["id"].as_str().unwrap_or_default().to_owned()
        } else {
            return Err(ChannelError::Address(exchange.to_owned()));
        };
        self.state().sent.insert(id.clone());
        Ok(id)
    }

    fn typing(&self, _exchange: &str) -> Result<(), ChannelError> {
        Ok(())
    }

    fn hook(&self, request: &HookRequest) -> HookAnswer {
        let secret = &self.keys.consumer_secret;
        match request.method {
            HookMethod::Get => crate::signature::x_crc(secret, request),
            HookMethod::Post if crate::signature::x(secret, request) => {
                let sent = self.state().sent.clone();
                self.report(activity(&request.body, &self.me, |id| sent.contains(id)), Delivery::Live);
                HookAnswer::Accepted
            }
            HookMethod::Post => HookAnswer::Refused,
        }
    }

    fn close(&self) {
        self.stopped.store(true, Ordering::Relaxed);
    }

    fn unlink(&self) {
        self.close();
    }
}

#[cfg(test)]
mod tests;
