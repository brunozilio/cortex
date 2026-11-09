//! Local models through MLX on Apple silicon: one runtime, served to the
//! native engine (ADR 0015), without a CLI protocol gateway.

use std::path::PathBuf;
use std::sync::{Arc, Weak};
use std::time::Duration;

use superfik_gateway::mlx::{MlxRuntime, Progress, SystemClock, SystemLauncher};
use superfik_protocol::DownloadState;

const REAP_EVERY: Duration = Duration::from_secs(60);

pub trait Downloads: Send + Sync {
    fn download(&self, id: &str, report: &mut dyn FnMut(DownloadState)) -> Result<(), String>;
}

impl Downloads for MlxRuntime {
    fn download(&self, id: &str, report: &mut dyn FnMut(DownloadState)) -> Result<(), String> {
        MlxRuntime::download(self, id, &mut |progress| {
            report(match progress {
                Progress::Preparing => DownloadState::Preparing,
                Progress::Fetching { done, total } => DownloadState::Fetching { done, total },
            })
        })
    }
}

pub(crate) fn download(downloads: Option<&Arc<dyn Downloads>>, id: String, outbox: &crate::outbox::Outbox) {
    use superfik_protocol::{ProviderEvent, ServerEvent};
    let say = |outbox: &crate::outbox::Outbox, id: &str, state| {
        outbox.send(ServerEvent::Providers(ProviderEvent::Download { id: id.to_owned(), state }));
    };
    let Some(downloads) = downloads.cloned() else {
        let why = "Local models need a Mac with Apple silicon (MLX).".to_owned();
        return say(outbox, &id, DownloadState::Failed(why));
    };
    say(outbox, &id, DownloadState::Started);
    let outbox = outbox.clone();
    let spawned = std::thread::Builder::new().name("superfik-model-download".into()).spawn(move || {
        let state = match downloads.download(&id, &mut |state| say(&outbox, &id, state)) {
            Ok(()) => DownloadState::Done,
            Err(why) => DownloadState::Failed(why),
        };
        say(&outbox, &id, state);
    });
    if let Err(error) = spawned {
        tracing::warn!(%error, "could not start the model download");
    }
}

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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use superfik_protocol::{ProviderEvent, ServerEvent};

    use super::*;
    use crate::outbox::Outbox;

    struct Fetches(Result<(), String>);

    impl Downloads for Fetches {
        fn download(&self, _id: &str, report: &mut dyn FnMut(DownloadState)) -> Result<(), String> {
            report(DownloadState::Fetching { done: 5, total: Some(10) });
            self.0.clone()
        }
    }

    fn states(downloads: Option<Arc<dyn Downloads>>) -> Vec<DownloadState> {
        let (tx, events) = crossbeam_channel::unbounded();
        download(downloads.as_ref(), "org/qwen".into(), &Outbox::new(tx, Arc::new(|| {})));
        let mut seen = Vec::new();
        while let Ok(ServerEvent::Providers(ProviderEvent::Download { id, state })) =
            events.recv_timeout(Duration::from_secs(2))
        {
            assert_eq!(id, "org/qwen");
            let last = !state.is_running();
            seen.push(state);
            if last {
                break;
            }
        }
        seen
    }

    #[test]
    fn a_download_says_it_started_how_it_goes_and_how_it_ended() {
        let half = DownloadState::Fetching { done: 5, total: Some(10) };
        let done = states(Some(Arc::new(Fetches(Ok(())))));
        assert_eq!(done, [DownloadState::Started, half.clone(), DownloadState::Done]);
        let failed = states(Some(Arc::new(Fetches(Err("disk full".into())))));
        assert_eq!(failed, [DownloadState::Started, half, DownloadState::Failed("disk full".into())]);
        assert!(matches!(states(None).as_slice(), [DownloadState::Failed(why)] if why.contains("Apple silicon")));
    }
}
