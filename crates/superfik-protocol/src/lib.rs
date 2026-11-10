//! Superfik client/backend protocol.
//!
//! The desktop UI never calls engines or touches the filesystem directly: it
//! sends [`ClientCommand`]s wrapped in an [`Envelope`] naming the acting
//! [`Principal`](superfik_core::Principal), and receives [`ServerEvent`]s.
//! Every type is serde-serializable, so the same messages work in-process
//! today and over a network connection to a remote server later.

mod addressed;
mod backend;
mod command;
mod envelope;
mod event;
mod provider_event;

pub use addressed::{Addressed, Recipient};
pub use backend::{Backend, BackendError, Wake};
pub use command::ClientCommand;
pub use envelope::Envelope;
pub use event::ServerEvent;
pub use provider_event::{DownloadState, ProviderEvent};

pub const PROTOCOL_VERSION: u32 = 1;
