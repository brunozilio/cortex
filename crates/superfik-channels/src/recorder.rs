//! A channel for tests: it records what is sent and lets the test play
//! what the account would report.

use std::sync::{Arc, Mutex};

use superfik_core::channel::ChannelKind;

use crate::{Channel, ChannelError, ChannelEvent, Events, HookAnswer, HookRequest, LinkSpec, Opener};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sent {
    Text { exchange: String, text: String },
    File { exchange: String, filename: Option<String>, size: usize, caption: String },
    Typing { exchange: String },
    Closed,
    Unlinked,
}

#[derive(Clone, Default)]
pub struct Recorder {
    kind: Option<ChannelKind>,
    sent: Arc<Mutex<Vec<Sent>>>,
    events: Arc<Mutex<Option<Events>>>,
    opened: Arc<Mutex<Vec<LinkSpec>>>,
    hooks: Arc<Mutex<Vec<HookRequest>>>,
}

impl std::fmt::Debug for Recorder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Recorder").field("sent", &self.sent()).finish_non_exhaustive()
    }
}

impl Recorder {
    pub fn of(kind: ChannelKind) -> Self {
        Self { kind: Some(kind), ..Self::default() }
    }

    pub fn hooks(&self) -> Vec<HookRequest> {
        self.hooks.lock().map(|hooks| hooks.clone()).unwrap_or_default()
    }

    pub fn sent(&self) -> Vec<Sent> {
        self.sent.lock().map(|sent| sent.clone()).unwrap_or_default()
    }

    pub fn opened(&self) -> Vec<LinkSpec> {
        self.opened.lock().map(|opened| opened.clone()).unwrap_or_default()
    }

    pub fn play(&self, event: ChannelEvent) {
        let events = self.events.lock().ok().and_then(|events| events.clone());
        if let Some(events) = events {
            events(event);
        }
    }

    fn push(&self, sent: Sent) {
        if let Ok(mut all) = self.sent.lock() {
            all.push(sent);
        }
    }
}

impl Opener for Recorder {
    fn kind(&self) -> ChannelKind {
        self.kind.unwrap_or(ChannelKind::WhatsappWeb)
    }

    fn open(&self, spec: &LinkSpec, events: Events) -> Result<Arc<dyn Channel>, ChannelError> {
        if let Ok(mut opened) = self.opened.lock() {
            opened.push(spec.clone());
        }
        if let Ok(mut slot) = self.events.lock() {
            *slot = Some(events);
        }
        Ok(Arc::new(self.clone()))
    }
}

impl Channel for Recorder {
    fn send_text(&self, exchange: &str, text: &str) -> Result<String, ChannelError> {
        self.push(Sent::Text { exchange: exchange.to_owned(), text: text.to_owned() });
        Ok(format!("sent-{}", self.sent().len()))
    }

    fn send_file(&self, exchange: &str, file: &crate::OutgoingFile<'_>) -> Result<String, ChannelError> {
        let (filename, size) = (file.media.filename.clone(), file.bytes.len());
        self.push(Sent::File { exchange: exchange.to_owned(), filename, size, caption: file.caption.to_owned() });
        Ok(format!("sent-{}", self.sent().len()))
    }

    fn typing(&self, exchange: &str) -> Result<(), ChannelError> {
        self.push(Sent::Typing { exchange: exchange.to_owned() });
        Ok(())
    }

    fn hook(&self, request: &HookRequest) -> HookAnswer {
        if let Ok(mut hooks) = self.hooks.lock() {
            hooks.push(request.clone());
        }
        HookAnswer::Accepted
    }

    fn close(&self) {
        self.push(Sent::Closed);
    }

    fn unlink(&self) {
        self.push(Sent::Unlinked);
    }
}
