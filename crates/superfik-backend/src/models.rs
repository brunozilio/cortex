//! Local models through MLX on Apple silicon: one runtime, served to the
//! native engine, its idle model let go in the background.

use std::path::PathBuf;
use std::sync::{Arc, Weak};
use std::time::Duration;

use superfik_mlx::{MlxRuntime, SystemClock, SystemLauncher};

const REAP_EVERY: Duration = Duration::from_secs(60);

#[derive(Debug, Default)]
pub(crate) struct LocalModels {
    pub runtime: Option<Arc<MlxRuntime>>,
    pub folder: Option<PathBuf>,
}

pub(crate) fn system() -> LocalModels {
    let apple_silicon = cfg!(target_os = "macos") && cfg!(target_arch = "aarch64");
    let Some(root) = crate::local_data_dir().filter(|_| apple_silicon) else {
        return LocalModels::default();
    };
    let folder = root.join("models").join("mlx");
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
