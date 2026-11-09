//! [`NativeEngine`]: Superfik's own engine behind the same
//! [`AgentEngine`] trait as the CLI adapters, so the reducer, the protocol
//! and the canvas see no difference.
//!
//! - `job`: a run's setup (session, brief, tools, agent, worktree, model).
//! - `converse`: its prompt, hooks, agent loop and saved conversation.
//! - `report`: side calls and how a run ends on the gate.

mod converse;
mod date;
mod job;
mod report;

use std::sync::Arc;

use superfik_core::{
    Decision, EngineEvent, EngineKind, FailureKind, ModelOption, PermissionMode, PromptRequest, RunId,
};
use superfik_engine::{AgentEngine, EngineError, EventSink, RunGate, RunHandle};

use self::job::Job;
use crate::control::Control;
use crate::model::http::{Http, UreqHttp};
use crate::model::{ModelApi, ModelError};
use crate::sessions::Sessions;
use crate::tools::Workspace;

const MAX_OUTPUT: u32 = 64_000;

pub trait ApiSource: Send + Sync {
    fn api(&self, request: &PromptRequest) -> Result<Arc<dyn ModelApi>, ModelError>;
}

pub struct NativeEngine {
    source: Arc<dyn ApiSource>,
    sessions: Arc<dyn Sessions>,
    models: Vec<ModelOption>,
    mcp: Arc<dyn Http>,
    worker: Option<std::path::PathBuf>,
}

impl std::fmt::Debug for NativeEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeEngine").field("models", &self.models).finish_non_exhaustive()
    }
}

impl NativeEngine {
    pub fn new(source: Arc<dyn ApiSource>, sessions: Arc<dyn Sessions>) -> Self {
        Self { source, sessions, models: models(), mcp: Arc::new(UreqHttp), worker: None }
    }

    pub fn with_worker(mut self, worker: std::path::PathBuf) -> Self {
        self.worker = Some(worker);
        self
    }
}

pub fn models() -> Vec<ModelOption> {
    Vec::new()
}

impl AgentEngine for NativeEngine {
    fn kind(&self) -> EngineKind {
        EngineKind::Native
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
        if request.model.engine != EngineKind::Native {
            return Err(EngineError::WrongEngine {
                model: request.model.id.clone(),
                expected: request.model.engine.label(),
                actual: EngineKind::Native.label(),
            });
        }
        if request.credentials.owner.as_ref().is_some_and(|owner| !owner.is_local()) && self.worker.is_none() {
            return Err(EngineError::Spawn {
                engine: EngineKind::Native.label(),
                source: std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "Native server runs require an isolated tool worker (SUPERFIK_NATIVE_WORKER).",
                ),
            });
        }
        let workspace = Arc::new(Workspace::for_request(request));
        let control = Arc::new(Control::new(request.permission, workspace.stop.clone()));
        let gate = RunGate::new(run, sink);
        let job = Job {
            worker: self.worker.clone(),
            source: Arc::clone(&self.source),
            sessions: Arc::clone(&self.sessions),
            mcp: Arc::clone(&self.mcp),
            request: request.clone(),
            workspace,
            control: Arc::clone(&control),
            gate: gate.clone(),
        };
        let panic_gate = gate.clone();
        let panic_control = control.clone();
        std::thread::Builder::new()
            .name("superfik-native-run".into())
            .spawn(move || {
                if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| job.run())).is_err() {
                    panic_control.cancel();
                    panic_gate.emit(EngineEvent::Failure(FailureKind::Other));
                    panic_gate.emit(EngineEvent::Failed("The native engine stopped unexpectedly.".into()));
                }
            })
            .map_err(|source| EngineError::Spawn { engine: EngineKind::Native.label(), source })?;
        Ok(Box::new(Handle { control }))
    }
}

struct Handle {
    control: Arc<Control>,
}

impl RunHandle for Handle {
    fn cancel(&mut self) {
        self.control.cancel();
    }

    fn answer(&mut self, ask: &superfik_core::AskId, decision: &Decision) {
        self.control.answer(ask, decision.clone());
    }

    fn set_mode(&mut self, mode: PermissionMode) {
        self.control.set_mode(mode);
    }
}
