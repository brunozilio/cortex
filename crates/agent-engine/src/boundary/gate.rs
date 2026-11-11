use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use domain::{EngineEvent, RunId};

use super::EventSink;

pub type Observer = Arc<dyn Fn(&EngineEvent) + Send + Sync>;

#[derive(Clone)]
pub struct RunGate {
    run: RunId,
    sink: Arc<dyn EventSink>,
    closed: Arc<AtomicBool>,
    observer: Arc<Mutex<Option<Observer>>>,
}

impl std::fmt::Debug for RunGate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunGate").field("run", &self.run).field("closed", &self.is_closed()).finish()
    }
}

impl RunGate {
    pub fn new(run: RunId, sink: Arc<dyn EventSink>) -> Self {
        Self { run, sink, closed: Arc::new(AtomicBool::new(false)), observer: Arc::default() }
    }

    pub fn observe(&self, observer: Observer) {
        if let Ok(mut slot) = self.observer.lock() {
            *slot = Some(observer);
        }
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
        let observer = self.observer.lock().ok().and_then(|slot| slot.clone());
        if let Some(observer) = observer {
            observer(&event);
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
    fn an_observer_sees_what_passes_the_gate_on_every_clone() {
        let (gate, _) = recording();
        let watched = Arc::new(Mutex::new(Vec::new()));
        let watch = Arc::clone(&watched);
        gate.clone().observe(Arc::new(move |event: &EngineEvent| watch.lock().expect("lock").push(event.clone())));
        gate.emit(EngineEvent::Cancelled);
        gate.emit(EngineEvent::TextDelta("late".into()));
        assert_eq!(*watched.lock().expect("lock"), vec![EngineEvent::Cancelled]);
    }

    #[test]
    fn clones_share_state() {
        let (gate, _) = recording();
        let other = gate.clone();
        other.emit(EngineEvent::Failed("x".into()));
        assert!(gate.is_closed());
    }
}
