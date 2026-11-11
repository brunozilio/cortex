mod authorizer;
mod channels;
mod dirs;
mod extensions;
mod features {
    include!(concat!(env!("OUT_DIR"), "/features.rs"));
}
mod handler;
mod local;
mod mcp_admin;
mod mcp_auth;
mod memory;
mod native;
mod outbox;
mod permissions;
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
pub use mcp_auth::McpAuth;
pub use memory::{InMemoryStore, Memories, MemoryStore, project_key};
pub use outbox::Outbox;
pub use protocol::{Backend, BackendError, Wake};
pub use providers::{InMemoryProviderStore, ModelProviders, ProviderStore};
pub use secrets::{InMemorySecretStore, SecretStore, Secrets};
pub use services::LocalServices;
pub use settings::SettingsStore;
