//! The Superfik server (ADR 0005, 0007, 0017). One per organization:
//! embedded in the desktop for local use, or hosted by a company. It runs
//! commands through the backend's `CommandHandler` and fans each event out
//! to the connections its [`Recipient`](superfik_protocol::Recipient)
//! reaches. Hosted, it keeps its own users (`superfik_identity`: a
//! first-access code, then a password; `sign_in`) and serves them over
//! `/ws` (`net`).

pub mod admin;
pub mod audit;
pub mod browser_pages;
pub mod checkouts;
pub mod cli;
pub mod comments;
pub mod config;
mod delegations;
pub mod github_login;
pub mod governance;
pub mod headers;
pub mod hooks;
pub mod hosting;
mod hub;
pub mod install;
pub mod library;
pub mod members;
pub mod net;
mod org;
mod presence;
pub mod project_sessions;
mod routine_budget;
mod server;
pub mod session;
pub mod settings;
pub mod shared_memory;
pub mod sign_in;
pub mod teams;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
mod uploads;
pub mod web_app;

pub use hub::{ConnectionId, Hub};
pub use net::{Authenticator, serve, socket, socket_checked_every, socket_for};
pub use org::{Clock, DirectoryRoles, Organization, origin_of, system_clock};
pub use server::{Connection, GitHubSignIn, Hosted, Server};
