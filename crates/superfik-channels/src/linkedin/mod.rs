//! A LinkedIn organization page, through the Community Management API.
//!
//! The account's fields are `organization_id` (the number in the page's
//! admin URL), `access_token` (a page admin's token with
//! `r_organization_social`, `w_organization_social` and
//! `rw_organization_admin`; the API is granted to approved apps only) and,
//! for the optional webhook, the app's `client_secret`. Linking reads the
//! organization's name. The page's latest posts and their comments are
//! read every `POLL` (the first time as history); a signed organization
//! notification on `/hooks/channels/<connection>` reads them at once. A
//! comment's conversation is answered by a reply under it; `feed`
//! publishes a post on the page. Members' messages are not open to apps.

mod parse;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

pub use parse::{COMMENT, Commented, FEED, comments, posts};
use serde_json::json;
use superfik_core::channel::{ChannelKind, Delivery};

use crate::http::{Http, HttpRequest, encode};
use crate::recent::Recent;
use crate::{Channel, ChannelError, ChannelEvent, Events, HookAnswer, HookMethod, HookRequest, LinkSpec, Opener};

pub const POLL: Duration = Duration::from_secs(300);
pub const VERSION: &str = "202410";
const WATCHED_POSTS: usize = 10;

pub struct LinkedinOpener {
    http: Arc<dyn Http>,
    api: String,
    poll: Option<Duration>,
}

impl std::fmt::Debug for LinkedinOpener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LinkedinOpener").field("api", &self.api).finish_non_exhaustive()
    }
}

impl LinkedinOpener {
    pub fn new(http: Arc<dyn Http>) -> Self {
        Self { http, api: "https://api.linkedin.com/rest".into(), poll: Some(POLL) }
    }

    pub fn at(http: Arc<dyn Http>, api: &str) -> Self {
        Self { http, api: api.trim_end_matches('/').to_owned(), poll: None }
    }

    pub fn link(&self, spec: &LinkSpec, events: Events) -> Result<Arc<LinkedinChannel>, ChannelError> {
        let field = |key: &str| {
            spec.credentials
                .get(key)
                .map(str::to_owned)
                .ok_or_else(|| ChannelError::Failed(format!("the account has no {key}")))
        };
        let id = field("organization_id")?;
        let channel = LinkedinChannel {
            organization: format!("urn:li:organization:{id}"),
            token: field("access_token")?,
            client_secret: spec.credentials.get("client_secret").map(str::to_owned),
            http: Arc::clone(&self.http),
            api: self.api.clone(),
            events,
            state: Mutex::default(),
            polling: Mutex::new(()),
            stopped: AtomicBool::new(false),
        };
        let found = channel.get(&format!("{}/organizations/{id}", self.api))?;
        let name = found["localizedName"].as_str().map_or_else(|| channel.organization.clone(), str::to_owned);
        (channel.events)(ChannelEvent::Linked { account: name });
        Ok(Arc::new(channel))
    }
}

impl Opener for LinkedinOpener {
    fn kind(&self) -> ChannelKind {
        ChannelKind::Linkedin
    }

    fn open(&self, spec: &LinkSpec, events: Events) -> Result<Arc<dyn Channel>, ChannelError> {
        let channel = self.link(spec, events)?;
        if let Some(every) = self.poll {
            let polled = Arc::clone(&channel);
            std::thread::Builder::new()
                .name("superfik-linkedin".into())
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
    seen: Recent,
    posts: HashMap<String, String>,
    sent: Recent,
}

pub struct LinkedinChannel {
    organization: String,
    token: String,
    client_secret: Option<String>,
    http: Arc<dyn Http>,
    api: String,
    events: Events,
    state: Mutex<State>,
    polling: Mutex<()>,
    stopped: AtomicBool,
}

impl std::fmt::Debug for LinkedinChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LinkedinChannel").field("organization", &self.organization).finish_non_exhaustive()
    }
}

impl LinkedinChannel {
    fn signed(&self, request: HttpRequest) -> HttpRequest {
        request.bearer(&self.token).header("LinkedIn-Version", VERSION).header("X-Restli-Protocol-Version", "2.0.0")
    }

    fn get(&self, url: &str) -> Result<serde_json::Value, ChannelError> {
        self.http.call(self.signed(HttpRequest::get(url)))?.ok()
    }

    fn post(&self, url: &str, body: &serde_json::Value) -> Result<serde_json::Value, ChannelError> {
        self.http.call(self.signed(HttpRequest::json(url, body)))?.ok()
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn poll(&self) {
        let _one = self.polling.lock().unwrap_or_else(PoisonError::into_inner);
        let url = format!("{}/posts?q=author&author={}&count={WATCHED_POSTS}", self.api, encode(&self.organization));
        let listed = match self.get(&url) {
            Ok(body) => posts(&body),
            Err(error) => return tracing::warn!(%error, "could not read the LinkedIn page's posts"),
        };
        let delivery = if self.state().read { Delivery::Live } else { Delivery::Backlog };
        let sent = self.state().sent.clone();
        for post in listed {
            let url = format!("{}/socialActions/{}/comments", self.api, encode(&post));
            let body = match self.get(&url) {
                Ok(body) => body,
                Err(error) => {
                    tracing::warn!(%error, "could not read a LinkedIn post's comments");
                    continue;
                }
            };
            for Commented { message, post } in comments(&body, &post, &self.organization, |id| sent.contains(id)) {
                {
                    let mut state = self.state();
                    if !state.seen.insert(message.id.clone()) {
                        continue;
                    }
                    state.posts.insert(message.exchange.clone(), post);
                }
                (self.events)(ChannelEvent::Message { message, delivery });
            }
        }
        self.state().read = true;
    }
}

impl Channel for LinkedinChannel {
    fn send_text(&self, exchange: &str, text: &str) -> Result<String, ChannelError> {
        let id = if exchange == FEED {
            let body = json!({
                "author": self.organization,
                "commentary": text,
                "visibility": "PUBLIC",
                "distribution": { "feedDistribution": "MAIN_FEED", "targetEntities": [], "thirdPartyDistributionChannels": [] },
                "lifecycleState": "PUBLISHED",
                "isReshareDisabledByAuthor": false,
            });
            self.post(&format!("{}/posts", self.api), &body)?;
            format!("post-{}", superfik_core::RunId::new())
        } else if let Some(comment) = exchange.strip_prefix(COMMENT) {
            let post = self
                .state()
                .posts
                .get(exchange)
                .cloned()
                .ok_or_else(|| ChannelError::Failed("that comment's post is not known yet".into()))?;
            let body = json!({
                "actor": self.organization,
                "object": post,
                "parentComment": comment,
                "message": { "text": text },
            });
            let made = self.post(&format!("{}/socialActions/{}/comments", self.api, encode(&post)), &body)?;
            made["$URN"].as_str().or(made["commentUrn"].as_str()).unwrap_or_default().to_owned()
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
        let Some(secret) = self.client_secret.as_deref() else { return HookAnswer::Refused };
        match request.method {
            HookMethod::Get => crate::signature::linkedin_challenge(secret, request),
            HookMethod::Post if crate::signature::linkedin(secret, request) => {
                self.poll();
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
