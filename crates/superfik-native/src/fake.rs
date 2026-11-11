//! A scripted engine for tests (feature `test-support`).
//!
//! [`FakeEngine`] replays a fixed list of events on a background thread,
//! optionally pausing between them, and honours cancellation exactly like a
//! real engine.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use superfik_core::{AskId, Decision, EngineEvent, ModelOption, PromptRequest, RunId};

use crate::boundary::{AgentEngine, EngineError, EventSink, RunGate, RunHandle};

#[derive(Debug, Clone)]
pub struct FakeEngine {
    models: Vec<ModelOption>,
    script: Vec<EngineEvent>,
    delay: Duration,
    answers: Arc<Mutex<Vec<(AskId, Decision)>>>,
    requests: Arc<Mutex<Vec<PromptRequest>>>,
    steers: Arc<Mutex<Vec<String>>>,
    steering: bool,
}

impl FakeEngine {
    pub fn new(models: Vec<ModelOption>, script: Vec<EngineEvent>) -> Self {
        Self {
            models,
            script,
            delay: Duration::ZERO,
            answers: Arc::default(),
            requests: Arc::default(),
            steers: Arc::default(),
            steering: true,
        }
    }

    /// The messages steered into its runs, each handed over at once.
    pub fn steers(&self) -> Vec<String> {
        self.steers.lock().map(|steers| steers.clone()).unwrap_or_default()
    }

    /// Like an engine that cannot take messages mid-run: it never hands
    /// them over.
    pub fn without_steering(mut self) -> Self {
        self.steering = false;
        self
    }

    pub fn requests(&self) -> Vec<PromptRequest> {
        self.requests.lock().map(|requests| requests.clone()).unwrap_or_default()
    }

    pub fn answers(&self) -> Vec<(AskId, Decision)> {
        self.answers.lock().map(|answers| answers.clone()).unwrap_or_default()
    }

    pub fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }

    pub fn demo() -> Self {
        let models = vec![ModelOption::new("claude-opus-5-5", "Opus 5.5")];
        let script = vec![
            EngineEvent::SessionStarted(superfik_core::SessionId("demo-session".into())),
            EngineEvent::ToolStarted(superfik_core::ToolCall::new("t1", "Read", "src/main.rs")),
            EngineEvent::ToolFinished { id: "t1".into() },
            EngineEvent::TextDelta("Here's a small night sky.\n\n".into()),
            EngineEvent::TextDelta("- One gradient\n- A glowing moon".into()),
            EngineEvent::Completed { duration_ms: 3_000, cost_usd: Some(0.01) },
        ];
        Self::new(models, script)
    }
}

impl AgentEngine for FakeEngine {
    fn models(&self) -> Vec<ModelOption> {
        self.models.clone()
    }

    fn start(
        &self,
        run: RunId,
        request: &PromptRequest,
        sink: Arc<dyn EventSink>,
    ) -> Result<Box<dyn RunHandle>, EngineError> {
        if let Ok(mut requests) = self.requests.lock() {
            requests.push(request.clone());
        }
        let gate = RunGate::new(run, sink);
        let cancelled = Arc::new(AtomicBool::new(false));
        let script = self.script.clone();
        let delay = self.delay;
        let replay_gate = gate.clone();
        let replay_cancelled = Arc::clone(&cancelled);
        thread::Builder::new()
            .name("superfik-fake-engine".into())
            .spawn(move || replay(script, delay, &replay_gate, &replay_cancelled))
            .map_err(|source| EngineError::Spawn { engine: "fake engine", source })?;
        let steers = self.steering.then(|| Arc::clone(&self.steers));
        Ok(Box::new(FakeHandle { gate, cancelled, answers: Arc::clone(&self.answers), steers }))
    }
}

fn replay(script: Vec<EngineEvent>, delay: Duration, gate: &RunGate, cancelled: &AtomicBool) {
    for event in script {
        if !delay.is_zero() {
            thread::sleep(delay);
        }
        if cancelled.load(Ordering::SeqCst) {
            return;
        }
        gate.emit(event);
    }
}

#[derive(Debug)]
struct FakeHandle {
    gate: RunGate,
    cancelled: Arc<AtomicBool>,
    answers: Arc<Mutex<Vec<(AskId, Decision)>>>,
    steers: Option<Arc<Mutex<Vec<String>>>>,
}

impl RunHandle for FakeHandle {
    fn steer(&mut self, text: String) {
        let Some(steers) = &self.steers else { return };
        if self.cancelled.load(Ordering::SeqCst) {
            return;
        }
        if let Ok(mut steers) = steers.lock() {
            steers.push(text.clone());
        }
        self.gate.emit(EngineEvent::Steered(text));
    }

    fn cancel(&mut self) {
        self.cancelled.store(true, Ordering::SeqCst);
        self.gate.emit(EngineEvent::Cancelled);
    }

    fn answer(&mut self, ask: &AskId, decision: &Decision) {
        if let Ok(mut answers) = self.answers.lock() {
            answers.push((ask.clone(), decision.clone()));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[derive(Debug, Default)]
    struct RecordingSink(Mutex<Vec<EngineEvent>>);

    impl RecordingSink {
        fn new() -> Arc<Self> {
            Arc::new(Self::default())
        }

        fn events(&self) -> Vec<EngineEvent> {
            self.0.lock().map(|events| events.clone()).unwrap_or_default()
        }

        fn wait_terminal(&self, timeout: Duration) -> Vec<EngineEvent> {
            let step = Duration::from_millis(2);
            let mut waited = Duration::ZERO;
            while waited < timeout && !self.events().iter().any(EngineEvent::is_terminal) {
                thread::sleep(step);
                waited += step;
            }
            self.events()
        }
    }

    impl EventSink for RecordingSink {
        fn emit(&self, _run: RunId, event: EngineEvent) {
            if let Ok(mut events) = self.0.lock() {
                events.push(event);
            }
        }
    }

    fn request() -> PromptRequest {
        PromptRequest::plain(PathBuf::from("/tmp"), "hi", ModelOption::new("claude-opus-5-5", "Opus 5.5"))
    }

    #[test]
    fn replays_script_in_order() {
        let sink = RecordingSink::new();
        let engine = FakeEngine::demo();
        engine.start(RunId::new(), &request(), sink.clone()).expect("start");
        let events = sink.wait_terminal(Duration::from_secs(2));
        assert_eq!(events, engine.script);
    }

    #[test]
    fn cancel_stops_replay_with_single_cancelled() {
        let sink = RecordingSink::new();
        let engine = FakeEngine::demo().with_delay(Duration::from_millis(20));
        let mut handle = engine.start(RunId::new(), &request(), sink.clone()).expect("start");
        handle.cancel();
        handle.cancel();
        thread::sleep(Duration::from_millis(60));
        let events = sink.events();
        assert_eq!(events.last(), Some(&EngineEvent::Cancelled));
        assert_eq!(events.iter().filter(|e| e.is_terminal()).count(), 1);
    }
}
