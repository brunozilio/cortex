//! Local models through MLX on Apple silicon: one runtime, served to the
//! native engine, its idle model let go in the background.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};
use std::time::Duration;

use superfik_mlx::{LocalModelError, MlxRuntime, SystemClock, SystemLauncher};

const REAP_EVERY: Duration = Duration::from_secs(60);

#[derive(Debug, Default)]
pub(crate) struct LocalModels {
    pub runtime: Option<Arc<MlxRuntime>>,
    pub folder: Option<PathBuf>,
}

const APPLE_SILICON: bool = cfg!(target_os = "macos") && cfg!(target_arch = "aarch64");

/// Where downloaded MLX models live: `<local data>/models/mlx`, on Apple
/// silicon only.
pub fn local_models_folder() -> Result<PathBuf, LocalModelError> {
    if !APPLE_SILICON {
        return Err(LocalModelError::NeedsAppleSilicon);
    }
    let root = crate::local_data_dir().ok_or(LocalModelError::NoDataFolder)?;
    Ok(models_in(&root))
}

fn models_in(root: &Path) -> PathBuf {
    root.join("models").join("mlx")
}

pub(crate) fn system() -> LocalModels {
    let Some(root) = crate::local_data_dir().filter(|_| APPLE_SILICON) else {
        return LocalModels::default();
    };
    let folder = models_in(&root);
    let launcher = Arc::new(SystemLauncher::new(&root.join("mlx")));
    let runtime = Arc::new(MlxRuntime::new(folder.clone(), launcher, Arc::new(SystemClock::default())));
    reap_while_alive(Arc::downgrade(&runtime));
    LocalModels { runtime: Some(runtime), folder: Some(folder) }
}

fn reap_while_alive(runtime: Weak<MlxRuntime>) {
    let spawned = std::thread::Builder::new().name("superfik-mlx-reaper".into()).spawn(move || {
        loop {
            std::thread::sleep(REAP_EVERY);
            match runtime.upgrade() {
                Some(runtime) => runtime.reap(),
                None => return,
            }
        }
    });
    if let Err(error) = spawned {
        tracing::warn!(%error, "idle local models will stay loaded");
    }
}
