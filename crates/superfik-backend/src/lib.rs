//! Superfik backend.
//!
//! A client talks to a [`Backend`]: it sends [`ClientCommand`]s in an
//! [`Envelope`] and polls [`ServerEvent`]s, woken by a [`Wake`] callback.
//! [`LocalBackend`] runs everything in-process; a future remote backend
//! implements the same trait over the network and reuses
//! [`CommandHandler`] with a real [`Authorizer`].
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
mod history;
mod local;
mod loopback;
mod memory;
mod models;
mod native;
mod outbox;
mod plugins;
mod providers;
mod runs;
mod sent;
mod services;
mod settings;
mod workspace;

pub use authorizer::{AuthError, Authorizer, LocalAuthorizer, forbidden};
pub use dirs::{app_data_dir, local_data_dir};
pub use extensions::Extensions;
pub use handler::CommandHandler;
pub use history::{FileSessionStore, InMemorySessionStore, SessionStore, Sessions};
pub use local::{LocalBackend, Workers};
pub use memory::{Embedder, FileMemoryStore, InMemoryStore, Memories, MemoryStore, ModelChoice, Purpose, project_key};
pub use native::engine as native_engine;
pub use outbox::Outbox;
pub use plugins::{
    FilePluginStore, InMemoryPluginStore, InMemorySecretStore, KeychainSecretStore, McpOAuth, OAuthFlow, OAuthRequest,
    PluginStore, Plugins, SealedSecretStore, SecretStore, Secrets, SetupRunner, SystemSetup, SystemTokenCommand,
    TokenCommand,
};
pub use providers::{
    BrowserSignIn, FileProviderStore, InMemoryProviderStore, ModelCatalog, ModelProviders, ProviderCatalog,
    ProviderStore, SignIn,
};
pub use services::LocalServices;
pub use settings::{FileSettingsStore, InMemorySettingsStore, SettingsStore};
pub use superfik_protocol::{Backend, BackendError, Wake};
