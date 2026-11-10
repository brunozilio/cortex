//! [`MlxRuntime`]: where a local model is served. One model is held in
//! memory at a time (a 27B model takes tens of gigabytes), served by its
//! own process, and let go after [`IDLE`].

use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use super::{Clock, Launcher, Served};

pub const IDLE: Duration = Duration::from_secs(15 * 60);
pub const READY_WAIT: Duration = Duration::from_secs(60);
const READY_POLL: Duration = Duration::from_millis(250);

struct Running {
    model: String,
    served: Box<dyn Served>,
    ready: bool,
    last_used: Duration,
}

pub struct MlxRuntime {
    models: PathBuf,
    launcher: Arc<dyn Launcher>,
    clock: Arc<dyn Clock>,
    running: Mutex<Option<Running>>,
}

impl std::fmt::Debug for MlxRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MlxRuntime").field("models", &self.models).finish_non_exhaustive()
    }
}

impl MlxRuntime {
    pub fn new(models: PathBuf, launcher: Arc<dyn Launcher>, clock: Arc<dyn Clock>) -> Self {
        Self { models, launcher, clock, running: Mutex::new(None) }
    }

    pub fn reap(&self) {
        let mut running = self.running.lock().unwrap_or_else(PoisonError::into_inner);
        let now = self.clock.now();
        if running.as_ref().is_some_and(|r| now.saturating_sub(r.last_used) >= IDLE)
            && let Some(mut idle) = running.take()
        {
            tracing::info!(model = %idle.model, "letting an idle local model go");
            idle.served.stop();
        }
    }

    fn folder(&self, model: &str) -> Option<PathBuf> {
        let path = Path::new(model);
        let plain = path.components().all(|c| matches!(c, Component::Normal(_)));
        (plain && !model.is_empty()).then(|| self.models.join(path))
    }

    fn wait_ready(&self, running: &mut Running) -> bool {
        let deadline = self.clock.now() + READY_WAIT;
        while !running.served.is_ready() {
            if self.clock.now() >= deadline || !running.served.is_running() {
                return false;
            }
            self.clock.sleep(READY_POLL);
        }
        true
    }
}

/// A model being served: its Chat Completions server and the name it
/// answers to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    pub base_url: String,
    pub model: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MlxError {
    /// Starting; ask again shortly.
    #[error("{0}")]
    Starting(String),
    #[error("{0}")]
    NotDownloaded(String),
    #[error("{0}")]
    Failed(String),
}

impl MlxRuntime {
    /// Serves `model` (stopping the one served before), installing MLX the
    /// first time.
    pub fn endpoint(&self, model: &str) -> Result<Endpoint, MlxError> {
        let folder = self
            .folder(model)
            .filter(|folder| folder.join("config.json").is_file())
            .ok_or_else(|| MlxError::NotDownloaded(format!("{model} is not downloaded")))?;
        let mut running = self.running.lock().unwrap_or_else(PoisonError::into_inner);
        let keep = running.as_mut().is_some_and(|r| r.model == model && r.served.is_running());
        if !keep {
            if let Some(mut previous) = running.take() {
                previous.served.stop();
            }
            if !self.launcher.is_installed() {
                tracing::info!("installing the MLX runtime");
                self.launcher.install().map_err(|why| MlxError::Failed(format!("Could not install MLX: {why}")))?;
            }
            let served = self
                .launcher
                .launch(&folder)
                .map_err(|error| MlxError::Failed(format!("Could not start {model}: {error}")))?;
            *running = Some(Running { model: model.to_owned(), served, ready: false, last_used: self.clock.now() });
        }
        let Some(current) = running.as_mut() else {
            return Err(MlxError::Failed(format!("Could not start {model}")));
        };
        current.last_used = self.clock.now();
        if !current.ready {
            current.ready = self.wait_ready(current);
        }
        if !current.ready {
            return Err(MlxError::Starting(format!("{model} is still starting")));
        }
        Ok(Endpoint { base_url: current.served.base_url(), model: folder.to_string_lossy().into_owned() })
    }
}

impl Drop for MlxRuntime {
    fn drop(&mut self) {
        let running = self.running.get_mut().unwrap_or_else(PoisonError::into_inner);
        if let Some(mut running) = running.take() {
            running.served.stop();
        }
    }
}
