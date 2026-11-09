//! Instagram direct messages and comments, through Meta's Graph API (the
//! Messenger Platform for Instagram).
//!
//! The account is a professional Instagram account linked to a Facebook
//! Page. Its fields are `page_id`, `page_access_token` (a Page token from a
//! long-lived user token, with `instagram_manage_messages` and
//! `instagram_manage_comments`; it does not expire), `app_secret` and
//! `verify_token`. Linking reads the Instagram account from the Page.
//! Messages and comments arrive through the webhook (fields `messages`,
//! `message_echoes` and `comments`). A DM is answered through
//! `/<page_id>/messages` (inside the 24-hour window); a comment by a reply
//! under it. Instagram sends files only from a public URL, so the agent
//! sends text.

mod parse;

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

pub use parse::{Arrived, COMMENT, parse};
use serde_json::json;
use superfik_core::channel::{ChannelKind, Delivery, Source};

use crate::http::{Http, HttpRequest};
use crate::meta::{self, GRAPH_VERSION};
use crate::{Bytes, Channel, ChannelError, ChannelEvent, Events, HookAnswer, HookRequest, LinkSpec, Opener};

pub struct InstagramOpener {
    http: Arc<dyn Http>,
    graph: String,
}

impl std::fmt::Debug for InstagramOpener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InstagramOpener").field("graph", &self.graph).finish_non_exhaustive()
    }
}

impl InstagramOpener {
    pub fn new(http: Arc<dyn Http>) -> Self {
        Self::at(http, &format!("https://graph.facebook.com/{GRAPH_VERSION}"))
    }

    pub fn at(http: Arc<dyn Http>, graph: &str) -> Self {
        Self { http, graph: graph.trim_end_matches('/').to_owned() }
    }
}

#[derive(Clone)]
struct Page {
    page_id: String,
    token: String,
    app_secret: String,
    verify_token: String,
}

impl Page {
    fn of(spec: &LinkSpec) -> Result<Self, ChannelError> {
        let field = |key: &str| {
            spec.credentials
                .get(key)
                .map(str::to_owned)
                .ok_or_else(|| ChannelError::Failed(format!("the account has no {key}")))
        };
        Ok(Self {
            page_id: field("page_id")?,
            token: field("page_access_token")?,
            app_secret: field("app_secret")?,
            verify_token: field("verify_token")?,
        })
    }
}

impl Opener for InstagramOpener {
    fn kind(&self) -> ChannelKind {
        ChannelKind::Instagram
    }

    fn open(&self, spec: &LinkSpec, events: Events) -> Result<Arc<dyn Channel>, ChannelError> {
        let page = Page::of(spec)?;
        let url = format!("{}/{}?fields=instagram_business_account{{id,username}}", self.graph, page.page_id);
        let found = self.http.call(HttpRequest::get(url).bearer(&page.token))?.ok()?;
        let linked = &found["instagram_business_account"];
        let account = linked["id"]
            .as_str()
            .ok_or_else(|| ChannelError::Failed("the Page has no professional Instagram account linked".into()))?
            .to_owned();
        let shown = linked["username"].as_str().map_or_else(|| account.clone(), |name| format!("@{name}"));
        events(ChannelEvent::Linked { account: shown });
        Ok(Arc::new(InstagramChannel {
            page,
            account,
            http: Arc::clone(&self.http),
            graph: self.graph.clone(),
            events,
            sent: Mutex::default(),
            names: Mutex::default(),
        }))
    }
}

struct InstagramChannel {
    page: Page,
    account: String,
    http: Arc<dyn Http>,
    graph: String,
    events: Events,
    sent: Mutex<HashSet<String>>,
    names: Mutex<HashMap<String, Option<String>>>,
}

impl InstagramChannel {
    fn post(&self, path: &str, body: &serde_json::Value) -> Result<serde_json::Value, ChannelError> {
        let url = format!("{}/{path}", self.graph);
        self.http.call(HttpRequest::json(url, body).bearer(&self.page.token))?.ok()
    }

    fn remember(&self, id: &str) {
        if let Ok(mut sent) = self.sent.lock() {
            sent.insert(id.to_owned());
        }
    }

    fn name_of(&self, igsid: &str) -> Option<String> {
        if let Some(known) = self.names.lock().ok().and_then(|names| names.get(igsid).cloned()) {
            return known;
        }
        let url = format!("{}/{igsid}?fields=name,username", self.graph);
        let found = self.http.call(HttpRequest::get(url).bearer(&self.page.token)).ok().and_then(|r| r.ok().ok());
        let name = found.and_then(|profile| {
            let name = profile["name"].as_str().filter(|n| !n.is_empty()).map(str::to_owned);
            name.or_else(|| profile["username"].as_str().map(|u| format!("@{u}")))
        });
        if let Ok(mut names) = self.names.lock() {
            names.insert(igsid.to_owned(), name.clone());
        }
        name
    }

    fn take(&self, body: &[u8]) {
        let sent = self.sent.lock().map(|sent| sent.clone()).unwrap_or_default();
        for Arrived { mut message, file_url } in parse(body, &self.account, |id| sent.contains(id)) {
            if message.source == Source::Contact && message.sender_name.is_none() {
                message.sender_name = self.name_of(&message.exchange);
            }
            if let (Some(media), Some(url)) = (message.media.clone(), file_url)
                && let Some(bytes) = meta::download(self.http.as_ref(), &url, None)
            {
                (self.events)(ChannelEvent::File { message: message.id.clone(), media, bytes: Bytes(bytes) });
            }
            (self.events)(ChannelEvent::Message { message, delivery: Delivery::Live });
        }
    }
}

impl Channel for InstagramChannel {
    fn send_text(&self, exchange: &str, text: &str) -> Result<String, ChannelError> {
        let id = if let Some(comment) = exchange.strip_prefix(COMMENT) {
            let replied = self.post(&format!("{comment}/replies"), &json!({ "message": text }))?;
            replied["id"].as_str().unwrap_or_default().to_owned()
        } else {
            if exchange.is_empty() || !exchange.bytes().all(|b| b.is_ascii_digit()) {
                return Err(ChannelError::Address(exchange.to_owned()));
            }
            let body = json!({ "recipient": { "id": exchange }, "message": { "text": text } });
            let sent = self.post(&format!("{}/messages", self.page.page_id), &body)?;
            sent["message_id"].as_str().unwrap_or_default().to_owned()
        };
        self.remember(&id);
        Ok(id)
    }

    fn typing(&self, exchange: &str) -> Result<(), ChannelError> {
        if exchange.starts_with(COMMENT) {
            return Ok(());
        }
        let body = json!({ "recipient": { "id": exchange }, "sender_action": "typing_on" });
        self.post(&format!("{}/messages", self.page.page_id), &body).map(|_| ())
    }

    fn hook(&self, request: &HookRequest) -> HookAnswer {
        meta::hook(&self.page.app_secret, &self.page.verify_token, request, |body| self.take(body))
    }

    fn close(&self) {}

    fn unlink(&self) {}
}

#[cfg(test)]
mod tests;
