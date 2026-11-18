pub mod agent;
pub mod agent_runtime;
mod boundary;
pub mod brief;
pub mod checkpoints;
pub mod chores;
pub mod config_schema;
pub mod control;
pub mod declared_providers;
mod engine;
#[cfg(any(test, feature = "test-support"))]
pub mod fake;
pub mod frontmatter;
pub mod guard;
pub mod hooks;
pub use language_servers as lsp;
mod lsp_fetch;
pub use lsp_fetch::NetFetch;
pub mod mcp;
pub mod meter;
pub mod plugins;
pub mod prompt;
pub mod sessions;
pub mod team;
pub mod tools;
pub mod trust;
pub mod upkeep;

#[cfg(test)]
#[cfg(unix)]
#[path = "hostile_repository_tests.rs"]
mod hostile_repository;

pub use llm_providers::{model, providers, transcript};

pub use boundary::{AgentEngine, EngineError, EventSink, RunGate, RunHandle};
pub use engine::date::today;
pub use engine::{ApiSource, Engine};
