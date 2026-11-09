//! Superfik agent engines.
//!
//! An [`AgentEngine`] turns a [`PromptRequest`](superfik_core::PromptRequest)
//! into a stream of [`EngineEvent`](superfik_core::EngineEvent)s delivered to
//! an [`EventSink`]. Engines are looked up by kind in an [`EngineRegistry`],
//! so adding Codex, Kiro, Copilot or Superfik's native engine is a matter of
//! registering another implementation (see `docs/architecture.md`).

mod engine;
#[cfg(any(test, feature = "fake"))]
pub mod fake;
mod gate;
pub mod locate;
pub mod planned;
mod registry;

pub use engine::{AgentEngine, EngineError, EventSink, RunHandle};
pub use gate::RunGate;
pub use planned::PlannedEngine;
pub use registry::EngineRegistry;
