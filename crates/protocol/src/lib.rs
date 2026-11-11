//! The app's client/backend protocol: a client sends [`ClientCommand`]s
//! in an [`Envelope`] naming the acting [`Principal`](domain::Principal)
//! and receives [`ServerEvent`]s. Everything is serde-serializable, so the
//! same messages go in process or over the daemon's socket.

mod backend;
mod command;
mod envelope;
mod event;
mod provider_event;

pub use backend::{Backend, BackendError, Wake};
pub use command::ClientCommand;
pub use envelope::Envelope;
pub use event::ServerEvent;
pub use provider_event::ProviderEvent;
