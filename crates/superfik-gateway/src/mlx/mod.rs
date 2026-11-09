//! Open models on Apple silicon through MLX (ADR 0015): `mlx-lm` in an
//! environment of the app's own, one `mlx_lm.server` per model in use.

use std::io;
use std::path::Path;
use std::time::Duration;

mod models;
mod runtime;
mod system;
mod tethered;
mod uv;

pub use models::{downloaded, repository_size};
pub use runtime::{IDLE, MlxRuntime, READY_WAIT};
pub use system::{SystemClock, SystemLauncher};

pub trait Launcher: Send + Sync {
    fn is_installed(&self) -> bool;
    fn install(&self) -> Result<(), String>;
    fn download(&self, id: &str, dest: &Path, tick: &mut dyn FnMut()) -> Result<(), String>;
    fn size(&self, id: &str) -> Option<u64>;
    fn launch(&self, model_dir: &Path) -> io::Result<Box<dyn Served>>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    Preparing,
    Fetching { done: u64, total: Option<u64> },
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
