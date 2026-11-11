//! The app's native agent engine: the agent loop runs in the backend's own
//! process, calling the model providers' APIs over HTTP and running its
//! tools as Rust functions. No other agent program is involved.
//!
//! - [`transcript`]: the conversation, independent of any provider.
//! - [`model`]: each provider's API behind [`model::ModelApi`].

pub mod agent;
pub mod agent_runtime;
mod boundary;
pub mod brief;
pub mod checkpoints;
pub mod chores;
pub mod control;
mod engine;
#[cfg(any(test, feature = "test-support"))]
pub mod fake;
pub mod frontmatter;
pub mod guard;
pub mod hooks;
pub mod judge;
pub mod lsp;
pub mod mcp;
pub mod meter;
pub mod model;
pub mod permission;
pub mod plugins;
pub mod prompt;
pub mod sessions;
pub mod tools;
pub mod transcript;
pub mod upkeep;

pub use boundary::{AgentEngine, EngineError, EventSink, RunGate, RunHandle};
pub use engine::{ApiSource, NativeEngine};
