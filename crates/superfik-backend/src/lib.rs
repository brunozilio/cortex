//! Superfik backend.
//!
//! The UI talks to a [`Backend`]: it sends [`ClientCommand`]s in an
//! [`Envelope`] and polls [`ServerEvent`]s, woken by a [`Wake`] callback.
//! [`LocalBackend`] runs everything in-process. A future remote backend
//! implements the same trait over the network, and a server reuses
//! [`CommandHandler`] with a real [`Authorizer`].
//!
//! [`ClientCommand`]: superfik_protocol::ClientCommand
//! [`Envelope`]: superfik_protocol::Envelope
//! [`ServerEvent`]: superfik_protocol::ServerEvent

mod authorizer;
mod browser;
mod budget;
mod channels;
mod dirs;
mod github;
mod handler;
mod history;
mod local;
mod loopback;
mod memory;
mod models;
mod native;
mod notes;
mod outbox;
mod pages;
mod plugins;
mod providers;
mod routines;
mod runs;
mod sent;
mod services;
mod settings;
mod sign_in;
mod vault;
mod workspace;

pub use authorizer::{AuthError, Authorizer, LocalAuthorizer, RoleAuthorizer, Roles, forbidden};
pub use budget::{Budget, Unbounded};
pub use channels::{
    ChannelAccount, ChannelStore, Channels, FileChannelStore, InMemoryChannelStore, Inbound, hook_path,
    now_ms as channels_now_ms, sender::SystemClock as ChannelClock,
};
pub use dirs::{app_data_dir, local_data_dir};
pub use github::{GitHubTokens, InMemoryGitHubTokens, NoGitHubTokens};
pub use handler::CommandHandler;
pub use history::{FileSessionStore, InMemorySessionStore, SessionStore, Sessions};
pub use local::{LocalBackend, Workers};
pub use memory::{Embedder, FileMemoryStore, InMemoryStore, Memories, MemoryStore, ModelChoice, Purpose, project_key};
pub use native::engine as native_engine;
pub use notes::{FileNoteStore, InMemoryNoteStore, NoteStore, Notes};
pub use outbox::Outbox;
pub use pages::{NoPages, PageService};
pub use plugins::{
    FilePluginStore, InMemoryPluginStore, InMemorySecretStore, KeychainSecretStore, McpOAuth, OAuthFlow, OAuthRequest,
    PluginStore, Plugins, SealedSecretStore, SecretStore, Secrets, SetupRunner, SystemSetup, SystemTokenCommand,
    TokenCommand,
};
pub use providers::{FileProviderStore, InMemoryProviderStore, ModelProviders, ProviderStore};
pub use routines::{FileRoutineStore, InMemoryRoutineStore, Refusal as WebhookRefusal, RoutineStore, Routines};
pub use services::LocalServices;
pub use settings::{FileSettingsStore, InMemorySettingsStore, SettingsStore};
pub use superfik_channels::{HookAnswer, HookMethod, HookRequest};
pub use superfik_protocol::{Backend, BackendError, Wake};
pub use vault::{
    Bitwarden, BitwardenCli, CommandRunner, Item, Ran, ShellRunner, SystemBitwarden, VaultAccount, VaultProvider,
    VaultProviders,
};
