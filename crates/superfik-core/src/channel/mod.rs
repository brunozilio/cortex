//! Channels (ADR 0012): where a product talks to people outside the
//! organization (WhatsApp, e-mail, Instagram, X, LinkedIn). A channel is a
//! plugin capability; each number, mailbox or handle is a connection. This
//! module holds what every channel means, apart from any protocol: the
//! shape of a message, the pace and ceilings of a number's sends (`pace`,
//! `limits`, for WhatsApp by QR; each kind's own in `kind`) and
//! which incoming messages wake the agent (`Exchange`).
//!
//! Times are unix milliseconds.

mod book;
mod kind;
pub mod limits;
pub mod media;
pub mod pace;
mod tool;

pub use book::{ChannelBook, Draft, Thread};
pub use media::{Attachment, MAX_MEDIA_BYTES};
pub use tool::*;

pub use kind::{ChannelKind, MAX_CHANNEL_HOOK_BYTES};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaKind {
    Image,
    Audio,
    Video,
    Document,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Media {
    pub kind: MediaKind,
    #[serde(default)]
    pub mime_type: Option<String>,
    #[serde(default)]
    pub filename: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub file: Option<std::path::PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Contact,
    Agent,
    Member,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub exchange: String,
    #[serde(default)]
    pub is_group: bool,
    #[serde(default)]
    pub sender: Option<String>,
    #[serde(default)]
    pub sender_name: Option<String>,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub media: Option<Media>,
    #[serde(default)]
    pub mentioned_me: bool,
    #[serde(default)]
    pub reply_to_me: bool,
    pub at: i64,
    pub source: Source,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum LinkStatus {
    #[default]
    Opening,
    Pairing {
        qr: String,
    },
    Linked {
        account: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hook: Option<String>,
    },
    LoggedOut,
    Failed {
        reason: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivery {
    Live,
    Backlog,
}

pub fn is_news(delivery: Delivery, at: i64, offline_since: Option<i64>) -> bool {
    match delivery {
        Delivery::Live => true,
        Delivery::Backlog => offline_since.is_some_and(|since| at >= since),
    }
}

pub const PAUSE_MS: i64 = 120 * 60_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Answer,
    Keep,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Exchange {
    pub id: String,
    #[serde(default)]
    pub paused_until: Option<i64>,
    #[serde(default)]
    pub last_at: Option<i64>,
}

impl Exchange {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into(), ..Self::default() }
    }

    pub fn observe(&mut self, message: &Message, pause_ms: i64) -> Verdict {
        self.last_at = Some(self.last_at.map_or(message.at, |last| last.max(message.at)));
        match message.source {
            Source::Agent => Verdict::Keep,
            Source::Member => {
                let until = message.at + pause_ms;
                self.paused_until = Some(self.paused_until.map_or(until, |paused| paused.max(until)));
                Verdict::Keep
            }
            Source::Contact => {
                let paused = self.paused_until.is_some_and(|until| message.at < until);
                let meant = !message.is_group || message.mentioned_me || message.reply_to_me;
                if paused || !meant { Verdict::Keep } else { Verdict::Answer }
            }
        }
    }
}

#[cfg(test)]
mod tests;
