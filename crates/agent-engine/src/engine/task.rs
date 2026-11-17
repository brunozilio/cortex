use std::sync::Arc;

use domain::{EngineEvent, FailureKind};
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
        .map_err(|error| EngineError::Spawn { engine: "the native engine", source: std::io::Error::other(error) })
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
            gate.emit(EngineEvent::Failure(FailureKind::Other));
            gate.emit(EngineEvent::Failed("The native engine stopped unexpectedly.".into()));
        }
    })
}
