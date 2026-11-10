//! Superfik's backend: a client sends [`ClientCommand`]s in an
//! [`Envelope`] and polls [`ServerEvent`]s, woken by a [`Wake`] callback.
//! [`LocalBackend`] runs everything in process; the daemon serves one per
//! connected terminal on shared services.
//!
//! [`ClientCommand`]: superfik_protocol::ClientCommand
//! [`Envelope`]: superfik_protocol::Envelope
//! [`ServerEvent`]: superfik_protocol::ServerEvent

mod authorizer;
#[cfg(unix)]
pub mod daemon;
mod dirs;
mod extensions;
mod handler;
mod local;
mod mcp_admin;
mod memory;
mod models;
mod native;
mod outbox;
mod providers;
mod runs;
mod secrets;
mod sent;
mod services;
mod settings;

pub use authorizer::{AuthError, Authorizer, LocalAuthorizer, forbidden};
pub use dirs::local_data_dir;
pub use extensions::{Extensions, McpServerSpec};
pub use handler::CommandHandler;
pub use local::LocalBackend;
pub use memory::{InMemoryStore, Memories, MemoryStore, project_key};
pub use outbox::Outbox;
pub use providers::{InMemoryProviderStore, ModelProviders, ProviderStore};
pub use secrets::{InMemorySecretStore, SecretStore, Secrets};
pub use services::LocalServices;
pub use settings::SettingsStore;
pub use superfik_protocol::{Backend, BackendError, Wake};

/// Local MLX models pulled from Hugging Face and removed again, on this
/// computer only (Apple silicon).
pub mod local_models {
    pub use crate::models::local_models_folder as folder;
    pub use superfik_mlx::{HfHub, Hub, LocalModelError, PullProgress, Pulled, Removed, pull, remove};
}
