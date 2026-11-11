//! Open models on Apple silicon through MLX: `mlx-lm` in an environment of
//! its own, one `mlx_lm.server` for the model in use.

use std::io;
use std::path::Path;
use std::time::Duration;

mod models;
mod pull;
mod runtime;
mod system;
mod tethered;
mod uv;

pub use models::downloaded;
pub use pull::{
    DEFAULT_ORG, HfHub, Hub, LocalModelError, PullProgress, Pulled, RemoteFile, Removed, RepoId, pull, remove,
};
pub use runtime::{Endpoint, IDLE, MlxError, MlxRuntime, READY_WAIT};
pub use system::{SystemClock, SystemLauncher};

pub trait Launcher: Send + Sync {
    fn is_installed(&self) -> bool;
    fn install(&self) -> Result<(), String>;
    fn launch(&self, model_dir: &Path) -> io::Result<Box<dyn Served>>;
}

pub trait Served: Send {
    fn base_url(&self) -> String;
    fn is_ready(&mut self) -> bool;
    fn is_running(&mut self) -> bool;
    fn stop(&mut self);
}

pub trait Clock: Send + Sync {
    fn now(&self) -> Duration;
    fn sleep(&self, by: Duration);
}
