use std::sync::Arc;

use domain::failure::RunFailure;
use tokio::runtime::Handle;
use tokio::task::JoinHandle;

use super::job::Job;
use crate::RunGate;
use crate::boundary::EngineError;
use crate::control::Control;
use crate::tools::Workspace;

pub(super) fn runtime() -> Result<Handle, EngineError> {
    platform::runtime::shared()
        .map(|runtime| runtime.handle().clone())
        .map_err(|error| EngineError::Spawn { engine: "the engine", source: std::io::Error::other(error) })
}

pub(super) fn spawn(
    runtime: &Handle,
    job: Job,
    workspace: Workspace,
    control: Arc<Control>,
    gate: RunGate,
) -> JoinHandle<()> {
    runtime.spawn(async move {
        if tokio::task::spawn_blocking(move || job.run(workspace)).await.is_err() {
            control.cancel();
            for event in RunFailure::other("The engine stopped unexpectedly.").events() {
                gate.emit(event);
            }
        }
    })
}
