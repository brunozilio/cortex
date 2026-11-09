//! WhatsApp through Meta's official Cloud API.
//!
//! The account's fields are `phone_number_id` (WhatsApp Manager › API
//! setup), `access_token` (a system user's permanent token with
//! `whatsapp_business_messaging`), `app_secret` (the Meta app's, to check
//! webhook signatures) and `verify_token` (any phrase, typed again in the
//! app's webhook settings). Linking reads the number from the Graph API;
//! messages arrive only through the webhook (`/hooks/channels/<connection>`,
//! subscribed to `messages`, and to `smb_message_echoes` for a number also
//! on the Business app). Sends go to `/<phone_number_id>/messages`; a file
//! is uploaded to `/<phone_number_id>/media` first.

mod parse;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub use parse::{Arrived, parse};
use serde_json::json;
use superfik_core::channel::{ChannelKind, Delivery, MediaKind};

use crate::http::{FilePart, Http, HttpRequest};
use crate::meta::{self, GRAPH_VERSION};
use crate::{
    Bytes, Channel, ChannelError, ChannelEvent, Events, HookAnswer, HookRequest, LinkSpec, Opener, OutgoingFile,
};

pub struct CloudOpener {
    http: Arc<dyn Http>,
    graph: String,
}

impl std::fmt::Debug for CloudOpener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CloudOpener").field("graph", &self.graph).finish_non_exhaustive()
    }
}

impl CloudOpener {
    pub fn new(http: Arc<dyn Http>) -> Self {
        Self::at(http, &format!("https://graph.facebook.com/{GRAPH_VERSION}"))
    }

    pub fn at(http: Arc<dyn Http>, graph: &str) -> Self {
        Self { http, graph: graph.trim_end_matches('/').to_owned() }
    }
}

#[derive(Clone)]
struct Number {
    phone_number_id: String,
    access_token: String,
    app_secret: String,
    verify_token: String,
}

impl Number {
    fn of(spec: &LinkSpec) -> Result<Self, ChannelError> {
        let field = |key: &str| {
            spec.credentials
                .get(key)
                .map(str::to_owned)
                .ok_or_else(|| ChannelError::Failed(format!("the account has no {key}")))
        };
        Ok(Self {
            phone_number_id: field("phone_number_id")?,
            access_token: field("access_token")?,
            app_secret: field("app_secret")?,
            verify_token: field("verify_token")?,
        })
    }
}

impl Opener for CloudOpener {
    fn kind(&self) -> ChannelKind {
        ChannelKind::WhatsappCloud
    }

    fn open(&self, spec: &LinkSpec, events: Events) -> Result<Arc<dyn Channel>, ChannelError> {
        let number = Number::of(spec)?;
        let url = format!("{}/{}?fields=display_phone_number,verified_name", self.graph, number.phone_number_id);
        let found = self.http.call(HttpRequest::get(url).bearer(&number.access_token))?.ok()?;
        let shown = found["display_phone_number"].as_str().unwrap_or(&number.phone_number_id);
        let account: String = shown.chars().filter(char::is_ascii_digit).collect();
        events(ChannelEvent::Linked { account });
        Ok(Arc::new(CloudChannel {
            number,
            http: Arc::clone(&self.http),
            graph: self.graph.clone(),
            events,
            last_heard: Mutex::default(),
        }))
    }
}

struct CloudChannel {
    number: Number,
    http: Arc<dyn Http>,
    graph: String,
    events: Events,
    last_heard: Mutex<HashMap<String, String>>,
}

impl CloudChannel {
    fn messages_url(&self) -> String {
        format!("{}/{}/messages", self.graph, self.number.phone_number_id)
    }

    fn send(&self, to: &str, kind: &str, content: serde_json::Value) -> Result<String, ChannelError> {
        let digits: String = to.chars().filter(char::is_ascii_digit).collect();
        if digits.is_empty() {
            return Err(ChannelError::Address(to.to_owned()));
        }
        let body = json!({
            "messaging_product": "whatsapp",
            "recipient_type": "individual",
            "to": digits,
            "type": kind,
            kind: content,
        });
        let sent =
            self.http.call(HttpRequest::json(self.messages_url(), &body).bearer(&self.number.access_token))?.ok()?;
        Ok(sent["messages"][0]["id"].as_str().unwrap_or_default().to_owned())
    }

    fn take(&self, body: &[u8]) {
        for Arrived { mut message, media_id } in parse(body, &self.number.phone_number_id) {
            if message.source == superfik_core::channel::Source::Contact
                && let Ok(mut heard) = self.last_heard.lock()
            {
                heard.insert(message.exchange.clone(), message.id.clone());
            }
            if let (Some(media), Some(id)) = (message.media.as_mut(), media_id)
                && let Some(bytes) = self.fetch(&id, media)
            {
                let event = ChannelEvent::File { message: message.id.clone(), media: media.clone(), bytes };
                (self.events)(event);
            }
            (self.events)(ChannelEvent::Message { message, delivery: Delivery::Live });
        }
    }

    fn fetch(&self, id: &str, media: &mut superfik_core::channel::Media) -> Option<Bytes> {
        let url = format!("{}/{id}", self.graph);
        let found = self.http.call(HttpRequest::get(url).bearer(&self.number.access_token)).ok()?.ok().ok()?;
        media.size = found["file_size"].as_u64();
        if media.mime_type.is_none() {
            media.mime_type = found["mime_type"].as_str().map(str::to_owned);
        }
        let address = found["url"].as_str()?;
        meta::download(self.http.as_ref(), address, Some(&self.number.access_token)).map(Bytes)
    }
}

impl Channel for CloudChannel {
    fn send_text(&self, exchange: &str, text: &str) -> Result<String, ChannelError> {
        self.send(exchange, "text", json!({ "body": text, "preview_url": true }))
    }

    fn send_file(&self, exchange: &str, file: &OutgoingFile<'_>) -> Result<String, ChannelError> {
        let mime = file.media.mime_type.as_deref().unwrap_or("application/octet-stream");
        let filename = file.media.filename.as_deref().unwrap_or("file");
        let part = FilePart { field: "file", filename, mime, bytes: file.bytes };
        let url = format!("{}/{}/media", self.graph, self.number.phone_number_id);
        let fields = [("messaging_product", "whatsapp"), ("type", mime)];
        let request = HttpRequest::multipart(url, &fields, &part).bearer(&self.number.access_token);
        let uploaded = self.http.call(request)?.ok()?;
        let id = uploaded["id"].as_str().ok_or_else(|| ChannelError::Failed("the upload gave no id".into()))?;
        let (kind, mut content) = match file.media.kind {
            MediaKind::Image => ("image", json!({ "id": id })),
            MediaKind::Video => ("video", json!({ "id": id })),
            MediaKind::Audio => ("audio", json!({ "id": id })),
            MediaKind::Document => ("document", json!({ "id": id, "filename": filename })),
        };
        if file.media.kind != MediaKind::Audio && !file.caption.is_empty() {
            content["caption"] = json!(file.caption);
        }
        let sent = self.send(exchange, kind, content)?;
        if file.media.kind == MediaKind::Audio && !file.caption.is_empty() {
            self.send_text(exchange, file.caption)?;
        }
        Ok(sent)
    }

    fn typing(&self, exchange: &str) -> Result<(), ChannelError> {
        let last = self.last_heard.lock().ok().and_then(|heard| heard.get(exchange).cloned());
        let Some(message_id) = last else { return Ok(()) };
        let body = json!({
            "messaging_product": "whatsapp",
            "status": "read",
            "message_id": message_id,
            "typing_indicator": { "type": "text" },
        });
        self.http.call(HttpRequest::json(self.messages_url(), &body).bearer(&self.number.access_token))?.ok()?;
        Ok(())
    }

    fn hook(&self, request: &HookRequest) -> HookAnswer {
        meta::hook(&self.number.app_secret, &self.number.verify_token, request, |body| self.take(body))
    }

    fn close(&self) {}

    fn unlink(&self) {}
}

#[cfg(test)]
mod tests;
