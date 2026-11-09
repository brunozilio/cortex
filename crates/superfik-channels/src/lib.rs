//! Channels (ADR 0012): the IO behind a channel plugin's accounts.
//!
//! An [`Opener`] links one account (a WhatsApp number, a mailbox) and
//! reports what happens on it as [`ChannelEvent`]s: a QR code to pair, the
//! account linked, a message (in `superfik_core::channel`'s shape, whatever
//! the protocol), the link closed. The [`Channel`] it returns sends. What a
//! message means for the agent, the pace and ceilings of sends, live in
//! `superfik_core::channel`; this crate only moves bytes.
//!
//! [`Recorder`] stands in for a real channel in tests. The WhatsApp adapter
//! (feature `whatsapp`, on by default) runs whatsapp-rust on its own Tokio
//! runtime, so callers stay synchronous like the rest of the backend.

mod address;
#[cfg(feature = "email")]
pub mod email;
mod hook;
#[cfg(feature = "official")]
pub mod http;
#[cfg(feature = "official")]
pub mod instagram;
#[cfg(feature = "official")]
pub mod linkedin;
#[cfg(feature = "official")]
mod meta;
#[cfg(feature = "official")]
mod recent;
mod recorder;
#[cfg(feature = "official")]
pub mod signature;
#[cfg(feature = "whatsapp")]
pub mod whatsapp;
#[cfg(feature = "official")]
pub mod whatsapp_cloud;
#[cfg(feature = "official")]
pub mod x;

use std::path::PathBuf;
use std::sync::Arc;

pub use address::{addressed_to, exchange_of, recipient_of};
pub use hook::{HookAnswer, HookMethod, HookRequest};
pub use recorder::{Recorder, Sent};
use superfik_core::ConnectionId;
use superfik_core::channel::{ChannelKind, Delivery, Media, Message};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkSpec {
    pub connection: ConnectionId,
    pub kind: ChannelKind,
    pub data_dir: PathBuf,
    pub credentials: Credentials,
}

#[derive(Clone, Default, PartialEq, Eq)]
pub struct Credentials(std::collections::BTreeMap<String, String>);

impl Credentials {
    pub fn new(values: std::collections::BTreeMap<String, String>) -> Self {
        Self(values)
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(|value| value.trim()).filter(|value| !value.is_empty())
    }
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_set().entries(self.0.keys()).finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChannelEvent {
    Qr { code: String, valid_secs: u64 },
    Linked { account: String },
    File { message: String, media: Media, bytes: Bytes },
    Message { message: Message, delivery: Delivery },
    Closed(CloseReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloseReason {
    LoggedOut,
    Failed(String),
}

pub type Events = Arc<dyn Fn(ChannelEvent) + Send + Sync>;

#[derive(Debug, thiserror::Error)]
pub enum ChannelError {
    #[error("the account is not connected yet")]
    NotConnected,
    #[error("`{0}` is not an address this channel can send to")]
    Address(String),
    #[error("{0}")]
    Failed(String),
}

#[derive(Clone, PartialEq, Eq)]
pub struct Bytes(pub Vec<u8>);

impl std::fmt::Debug for Bytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Bytes({})", self.0.len())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct OutgoingFile<'a> {
    pub media: &'a Media,
    pub bytes: &'a [u8],
    pub caption: &'a str,
}

pub trait Channel: Send + Sync {
    fn send_text(&self, exchange: &str, text: &str) -> Result<String, ChannelError>;
    fn send_file(&self, _exchange: &str, _file: &OutgoingFile<'_>) -> Result<String, ChannelError> {
        Err(ChannelError::Failed("this channel cannot send files".to_owned()))
    }
    fn typing(&self, exchange: &str) -> Result<(), ChannelError>;
    fn hook(&self, _request: &HookRequest) -> HookAnswer {
        HookAnswer::Refused
    }
    fn close(&self);
    fn unlink(&self);
}

pub trait Opener: Send + Sync {
    fn kind(&self) -> ChannelKind;
    fn open(&self, spec: &LinkSpec, events: Events) -> Result<Arc<dyn Channel>, ChannelError>;
}
