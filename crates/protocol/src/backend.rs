//! The client's boundary with its server: whatever carries commands to
//! one and events back (in process, or a socket). Here, beside the
//! messages, so a client depends on nothing of the server.

use std::sync::Arc;

use crate::{ClientCommand, Envelope, ServerEvent};

pub type Wake = Arc<dyn Fn() + Send + Sync>;

pub trait Backend: Send {
    fn send(&self, command: Envelope<ClientCommand>) -> Result<(), BackendError>;

    fn try_recv(&self) -> Option<ServerEvent>;
}

#[derive(Debug, thiserror::Error)]
pub enum BackendError {
    #[error("the backend has shut down")]
    Disconnected,
    #[error("failed to start backend worker: {0}")]
    Worker(#[source] std::io::Error),
}
