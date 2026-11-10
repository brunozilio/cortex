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
mod loopback;
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
pub use dirs::{app_data_dir, local_data_dir};
pub use extensions::{Extensions, McpServerSpec};
pub use handler::CommandHandler;
pub use local::{LocalBackend, Workers};
pub use memory::{Embedder, FileMemoryStore, InMemoryStore, Memories, MemoryStore, ModelChoice, Purpose, project_key};
pub use native::engine as native_engine;
pub use outbox::Outbox;
pub use providers::{
    BrowserSignIn, FileProviderStore, InMemoryProviderStore, ModelCatalog, ModelProviders, ProviderCatalog,
    ProviderStore, SignIn,
};
pub use secrets::{InMemorySecretStore, SealedSecretStore, SecretStore, Secrets, system_secrets};
pub use services::LocalServices;
pub use settings::{FileSettingsStore, InMemorySettingsStore, SettingsStore};
pub use superfik_protocol::{Backend, BackendError, Wake};
