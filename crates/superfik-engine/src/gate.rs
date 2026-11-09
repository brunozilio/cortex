//! Guarantees each run ends with exactly one terminal event.
//!
//! A run can end from two directions at once: the engine finishing and the
//! user cancelling. Both paths go through a shared [`RunGate`]; whoever
//! closes it first emits the terminal event, and everything after is dropped.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use superfik_core::{EngineEvent, RunId};

use crate::engine::EventSink;

#[derive(Clone)]
pub struct RunGate {
    run: RunId,
    sink: Arc<dyn EventSink>,
    closed: Arc<AtomicBool>,
}

impl std::fmt::Debug for RunGate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunGate").field("run", &self.run).field("closed", &self.is_closed()).finish()
    }
}

impl RunGate {
    pub fn new(run: RunId, sink: Arc<dyn EventSink>) -> Self {
        Self { run, sink, closed: Arc::new(AtomicBool::new(false)) }
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    pub fn emit(&self, event: EngineEvent) -> bool {
        if event.is_terminal() {
            if self.closed.swap(true, Ordering::SeqCst) {
                return false;
            }
        } else if self.is_closed() {
            return false;
        }
        self.sink.emit(self.run, event);
        true
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    fn recording() -> (RunGate, Arc<Mutex<Vec<EngineEvent>>>) {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let record = Arc::clone(&seen);
        let sink = Arc::new(move |_run: RunId, event: EngineEvent| {
            if let Ok(mut events) = record.lock() {
                events.push(event);
            }
        });
        (RunGate::new(RunId::new(), sink), seen)
    }

    #[test]
    fn only_first_terminal_event_passes() {
        let (gate, seen) = recording();
        assert!(gate.emit(EngineEvent::TextDelta("a".into())));
        assert!(gate.emit(EngineEvent::Cancelled));
        assert!(!gate.emit(EngineEvent::Completed { duration_ms: 1, cost_usd: None }));
        assert!(!gate.emit(EngineEvent::TextDelta("late".into())));
        assert_eq!(*seen.lock().expect("lock"), vec![EngineEvent::TextDelta("a".into()), EngineEvent::Cancelled]);
    }

    #[test]
    fn clones_share_state() {
        let (gate, _) = recording();
        let other = gate.clone();
        other.emit(EngineEvent::Failed("x".into()));
        assert!(gate.is_closed());
    }
}
