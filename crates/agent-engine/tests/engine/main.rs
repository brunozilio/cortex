//! The native engine through `AgentEngine::start`, as the backend drives it.

mod agents;
mod brand_only;
mod checkpoints;
mod headless;
mod inbox;
mod lsp;
mod mcp;
mod plugins;
mod runs;
mod sessions;
mod steering;
mod verification;
mod worktrees;

use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use agent_engine::model::scripted::ScriptedModel;
use agent_engine::model::{ModelApi, ModelError};
use agent_engine::{AgentEngine, ApiSource, NativeEngine};
use domain::{EngineEvent, ModelOption, PromptRequest, RunId};

struct Source(Mutex<Vec<Arc<ScriptedModel>>>);

impl ApiSource for Source {
    fn api(&self, _request: &PromptRequest) -> Result<Arc<dyn ModelApi>, ModelError> {
        let mut models = self.0.lock().expect("models");
        if models.is_empty() {
            return Err(ModelError::Unauthorized("no account connected".into()));
        }
        Ok(models.remove(0))
    }
}

/// Runs a request with an empty home of its own, so the person's settings,
/// skills and MCP servers stay out of the tests.
fn start(engine: &NativeEngine, request: &PromptRequest) -> Receiver<EngineEvent> {
    start_with_handle(engine, request).0
}

/// [`start`], keeping the handle to steer, answer or cancel the run.
fn start_with_handle(
    engine: &NativeEngine,
    request: &PromptRequest,
) -> (Receiver<EngineEvent>, Box<dyn agent_engine::RunHandle>) {
    static HOME: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    let home = HOME.get_or_init(|| tempfile::tempdir().expect("home")).path().to_path_buf();
    let mut request = request.clone();
    let extensions = request.extensions.get_or_insert_with(Default::default);
    extensions.home.get_or_insert(home);
    let request = &request;
    let (send, receive) = channel();
    let send = Mutex::new(send);
    let sink = Arc::new(move |_run: RunId, event: EngineEvent| {
        let _ = send.lock().expect("send").send(event);
    });
    let handle = engine.start(RunId::new(), request, sink).expect("start");
    (receive, handle)
}

fn finish(events: &Receiver<EngineEvent>) -> Vec<EngineEvent> {
    let mut seen = Vec::new();
    while let Ok(event) = events.recv_timeout(Duration::from_secs(10)) {
        let terminal = event.is_terminal();
        seen.push(event);
        if terminal {
            break;
        }
    }
    seen
}

fn model() -> ModelOption {
    ModelOption::new("claude-opus-5-5", "Opus 5.5")
}
