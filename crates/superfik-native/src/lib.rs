//! Superfik's native agent engine: the agent loop runs in the backend's own
//! process, calling the model providers' APIs over HTTP and running its
//! tools as Rust functions. No CLI is involved.
//!
//! - [`transcript`]: the conversation, independent of any provider.
//! - [`model`]: each provider's API behind [`model::ModelApi`].

pub mod agent;
pub mod agent_runtime;
pub mod brief;
pub mod chores;
pub mod control;
mod engine;
pub mod guard;
pub mod hooks;
pub mod image;
pub mod mcp;
pub mod meter;
pub mod model;
pub mod permission;
pub mod prompt;
pub mod research;
pub mod sessions;
pub mod tools;
pub mod transcript;
pub mod upkeep;
pub mod worker;

pub use engine::{ApiSource, NativeEngine, models};
