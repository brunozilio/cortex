//! A page's passkey requests: the contexts they may come from (the page's
//! own world, by origin), the calls waiting for the app's answer, and the
//! answer sent back into the context that asked.

use std::collections::HashMap;
use std::hash::{BuildHasher, Hasher};

use serde_json::Value;

use super::Page;
use crate::PageEvent;
use crate::devtools;
use crate::passkey::{PasskeyAsk, PasskeyReply, answer_script};

pub(super) struct Passkeys {
    pub(super) answer: String,
    contexts: HashMap<i64, String>,
    pending: HashMap<u64, i64>,
}

impl Default for Passkeys {
    fn default() -> Self {
        let random = std::collections::hash_map::RandomState::new().build_hasher().finish();
        Self { answer: format!("__sfk{random:016x}"), contexts: HashMap::new(), pending: HashMap::new() }
    }
}

impl Page {
    pub(super) fn passkey_event(&mut self, method: &str, params: &Value, events: &mut Vec<PageEvent>) {
        if method == "Runtime.executionContextsCleared" {
            self.passkeys.contexts.clear();
            self.passkeys.pending.clear();
        }
        if let Some((id, origin)) = devtools::top_context(method, params) {
            self.passkeys.contexts.insert(id, origin);
        }
        let Some((context, payload)) = devtools::passkey_payload(method, params) else { return };
        let Some(origin) = self.passkeys.contexts.get(&context).cloned() else { return };
        let top = url::Url::parse(&self.url).ok().map(|u| u.origin().ascii_serialization());
        let Some(ask) = PasskeyAsk::read(&payload, &origin) else { return };
        self.passkeys.pending.insert(ask.request, context);
        if top.as_deref() == Some(origin.as_str()) {
            events.push(PageEvent::PasskeyRequested(ask));
        } else {
            self.answer_passkey(ask.request, &PasskeyReply::Elsewhere);
        }
    }

    pub(super) fn answer_passkey(&mut self, request: u64, reply: &PasskeyReply) {
        let Some(context) = self.passkeys.pending.remove(&request) else { return };
        let script = answer_script(&self.passkeys.answer, request, reply);
        let (_, message) = self.cdp.evaluate(context, &script);
        self.send_devtools(&message);
    }
}
