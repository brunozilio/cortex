//! A scripted engine for demos and tests (feature `fake`).
//!
//! [`FakeEngine`] replays a fixed list of events on a background thread,
//! optionally pausing between them, and honours cancellation exactly like a
//! real engine. [`RecordingSink`] collects what an engine emitted.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use superfik_core::{AskId, Decision, EngineEvent, EngineKind, ModelOption, PromptRequest, RunId};

use crate::engine::{AgentEngine, EngineError, EventSink, RunHandle};
use crate::gate::RunGate;

#[derive(Debug, Clone)]
pub struct FakeEngine {
    kind: EngineKind,
    models: Vec<ModelOption>,
    script: Vec<EngineEvent>,
    delay: Duration,
    answers: Arc<Mutex<Vec<(AskId, Decision)>>>,
    requests: Arc<Mutex<Vec<PromptRequest>>>,
}

impl FakeEngine {
    pub fn new(kind: EngineKind, models: Vec<ModelOption>, script: Vec<EngineEvent>) -> Self {
        Self { kind, models, script, delay: Duration::ZERO, answers: Arc::default(), requests: Arc::default() }
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
        let models = vec![ModelOption::new(EngineKind::Native, "claude-opus-5-5", "Opus 5.5")];
        let script = vec![
            EngineEvent::SessionStarted(superfik_core::SessionId("demo-session".into())),
            EngineEvent::ToolStarted(superfik_core::ToolCall::new("t1", "Read", "src/main.rs")),
            EngineEvent::ToolFinished { id: "t1".into() },
            EngineEvent::TextDelta("Here's a small night sky.\n\n".into()),
            EngineEvent::TextDelta("- One gradient\n- A glowing moon".into()),
            EngineEvent::Completed { duration_ms: 3_000, cost_usd: Some(0.01) },
        ];
        Self::new(EngineKind::Native, models, script)
    }
}

impl AgentEngine for FakeEngine {
    fn kind(&self) -> EngineKind {
        self.kind
    }

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
        Ok(Box::new(FakeHandle { gate, cancelled, answers: Arc::clone(&self.answers) }))
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
}

impl RunHandle for FakeHandle {
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

#[derive(Debug, Default)]
pub struct RecordingSink {
    events: Mutex<Vec<(RunId, EngineEvent)>>,
}

impl RecordingSink {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn events(&self) -> Vec<EngineEvent> {
        self.entries().into_iter().map(|(_, event)| event).collect()
    }

    pub fn entries(&self) -> Vec<(RunId, EngineEvent)> {
        self.events.lock().map(|events| events.clone()).unwrap_or_default()
    }

    pub fn wait_terminal(&self, timeout: Duration) -> Vec<EngineEvent> {
        let step = Duration::from_millis(2);
        let mut waited = Duration::ZERO;
        while waited < timeout {
            let events = self.events();
            if events.iter().any(EngineEvent::is_terminal) {
                return events;
            }
            thread::sleep(step);
            waited += step;
        }
        self.events()
    }
}

impl EventSink for RecordingSink {
    fn emit(&self, run: RunId, event: EngineEvent) {
        if let Ok(mut events) = self.events.lock() {
            events.push((run, event));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use superfik_core::{Effort, WorkspaceId};

    use super::*;

    pub(crate) fn request() -> PromptRequest {
        PromptRequest {
            connections: Vec::new(),
            plugins: None,
            credentials: Default::default(),
            workspace: WorkspaceId::new(),
            cwd: PathBuf::from("/tmp"),
            prompt: "hi".into(),
            model: ModelOption::new(EngineKind::Native, "claude-opus-5-5", "Opus 5.5"),
            effort: Effort::Medium,
            repositories: vec![],
            skills: vec![],
            disabled_skills: Vec::new(),
            references: vec![],
            resume: None,
            command: None,
            memory: None,
            permission: Default::default(),
            output_style: None,
            safety_check: true,
        }
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
