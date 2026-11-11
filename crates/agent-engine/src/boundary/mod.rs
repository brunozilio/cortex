//! The engine boundary: an [`AgentEngine`] turns a [`PromptRequest`] into
//! [`EngineEvent`]s delivered to an [`EventSink`]. [`NativeEngine`] implements
//! it; `fake` (feature `test-support`) is a scripted one for tests.
//!
//! [`NativeEngine`]: crate::NativeEngine

mod gate;

use std::sync::Arc;

pub use self::gate::RunGate;

use domain::{AskId, Decision, EngineEvent, ModelOption, PermissionMode, PromptRequest, RunId};

pub trait EventSink: Send + Sync {
    fn emit(&self, run: RunId, event: EngineEvent);
}

impl<F> EventSink for F
where
    F: Fn(RunId, EngineEvent) + Send + Sync,
{
    fn emit(&self, run: RunId, event: EngineEvent) {
        self(run, event);
    }
}

pub trait RunHandle: Send {
    fn cancel(&mut self);

    fn answer(&mut self, _ask: &AskId, _decision: &Decision) {}

    fn set_mode(&mut self, _mode: PermissionMode) {}

    /// Hands the model a message at its next turn. Engines that cannot
    /// never announce it, and the client sends it as the next prompt.
    fn steer(&mut self, _text: String) {}

    /// Takes back the steered messages not handed over yet, announced as
    /// `EngineEvent::Unsteered`.
    fn unsteer(&mut self) {}

    /// Moves the command the run is waiting on to the background (Ctrl+B).
    fn background(&mut self) {}
}

pub trait AgentEngine: Send + Sync {
    /// Models the engine serves by itself (the providers' come from their
    /// catalogs).
    fn models(&self) -> Vec<ModelOption>;

    fn start(
        &self,
        run: RunId,
        request: &PromptRequest,
        sink: Arc<dyn EventSink>,
    ) -> Result<Box<dyn RunHandle>, EngineError>;
}

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("failed to start {engine}: {source}")]
    Spawn {
        engine: &'static str,
        #[source]
        source: std::io::Error,
    },
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    #[test]
    fn closures_are_sinks() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let record = Arc::clone(&seen);
        let sink: Arc<dyn EventSink> = Arc::new(move |_run: RunId, event: EngineEvent| {
            if let Ok(mut events) = record.lock() {
                events.push(event);
            }
        });
        sink.emit(RunId::new(), EngineEvent::Cancelled);
        assert_eq!(*seen.lock().expect("lock"), vec![EngineEvent::Cancelled]);
    }
}
