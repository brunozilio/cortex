//! The agent engine boundary: an [`AgentEngine`] turns a
//! [`PromptRequest`](superfik_core::PromptRequest) into
//! [`EngineEvent`](superfik_core::EngineEvent)s delivered to an
//! [`EventSink`]. Superfik's native engine implements it; [`fake`] is a
//! scripted one for tests.

mod engine;
#[cfg(any(test, feature = "fake"))]
pub mod fake;
mod gate;

pub use engine::{AgentEngine, EngineError, EventSink, RunHandle};
pub use gate::RunGate;
