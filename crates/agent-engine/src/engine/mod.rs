//! [`NativeEngine`]: Superfik's own engine behind the [`AgentEngine`]
//! trait the backend drives.
//!
//! - `job`: a run's setup (session, brief, tools, agent, worktree, model).
//! - `converse`: its prompt, hooks, agent loop and saved conversation.
//! - `report`: side calls and how a run ends on the gate.
//! - `rewind`: each run's checkpoint, and going back to one.

mod converse;
mod date;
mod job;
mod moved;
mod report;
mod rewind;

use std::sync::Arc;

use self::job::Job;
use crate::boundary::{AgentEngine, EngineError, EventSink, RunGate, RunHandle};
use crate::control::Control;
use crate::lsp::{LanguageServers, LspPool};
use crate::model::http::{Http, NetHttp};
use crate::model::{ModelApi, ModelError};
use crate::sessions::Sessions;
use crate::tools::Workspace;
use domain::{Decision, EngineEvent, FailureKind, ModelOption, PermissionMode, PromptRequest, RunId};

const MAX_OUTPUT: u32 = 64_000;

pub trait ApiSource: Send + Sync {
    fn api(&self, request: &PromptRequest) -> Result<Arc<dyn ModelApi>, ModelError>;
}

pub struct NativeEngine {
    source: Arc<dyn ApiSource>,
    sessions: Arc<dyn Sessions>,
    mcp: Arc<dyn Http>,
    lsp: Arc<dyn LanguageServers>,
}

impl std::fmt::Debug for NativeEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeEngine").finish_non_exhaustive()
    }
}

impl NativeEngine {
    pub fn new(source: Arc<dyn ApiSource>, sessions: Arc<dyn Sessions>) -> Self {
        Self { source, sessions, mcp: Arc::new(NetHttp), lsp: Arc::new(LspPool::default()) }
    }

    /// The language servers runs use, kept for the engine's life; started
    /// as processes by default.
    pub fn with_language_servers(mut self, lsp: Arc<dyn LanguageServers>) -> Self {
        self.lsp = lsp;
        self
    }
}

impl AgentEngine for NativeEngine {
    fn models(&self) -> Vec<ModelOption> {
        Vec::new()
    }

    fn start(
        &self,
        run: RunId,
        request: &PromptRequest,
        sink: Arc<dyn EventSink>,
    ) -> Result<Box<dyn RunHandle>, EngineError> {
        let workspace = Arc::new(Workspace::for_request(request));
        let control = Arc::new(Control::new(request.permission, workspace.stop.clone()));
        let detach = Arc::clone(&workspace.detach);
        let gate = RunGate::new(run, sink);
        let job = Job {
            source: Arc::clone(&self.source),
            sessions: Arc::clone(&self.sessions),
            mcp: Arc::clone(&self.mcp),
            lsp: Arc::clone(&self.lsp),
            request: request.clone(),
            workspace,
            control: Arc::clone(&control),
            gate: gate.clone(),
        };
        let panic_gate = gate.clone();
        let panic_control = control.clone();
        std::thread::Builder::new()
            .name("agent-engine-run".into())
            .spawn(move || {
                if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| job.run())).is_err() {
                    panic_control.cancel();
                    panic_gate.emit(EngineEvent::Failure(FailureKind::Other));
                    panic_gate.emit(EngineEvent::Failed("The native engine stopped unexpectedly.".into()));
                }
            })
            .map_err(|source| EngineError::Spawn { engine: "the native engine", source })?;
        Ok(Box::new(Handle { control, gate, detach }))
    }
}

struct Handle {
    control: Arc<Control>,
    gate: RunGate,
    detach: Arc<std::sync::atomic::AtomicBool>,
}

impl RunHandle for Handle {
    fn cancel(&mut self) {
        self.control.cancel();
    }

    fn answer(&mut self, ask: &domain::AskId, decision: &Decision) {
        self.control.answer(ask, decision.clone());
    }

    fn set_mode(&mut self, mode: PermissionMode) {
        self.control.set_mode(mode);
    }

    fn steer(&mut self, text: String) {
        self.control.steer(text);
    }

    fn unsteer(&mut self) {
        self.gate.emit(EngineEvent::Unsteered(self.control.take_steered()));
    }

    fn background(&mut self) {
        self.detach.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}
