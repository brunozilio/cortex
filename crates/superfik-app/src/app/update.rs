//! Runs the updater for the reducer (`Effect::Update`) on threads: asks
//! the latest GitHub release's manifest at launch and every `CHECK_EVERY`,
//! downloads, checks and stages the offered version with its progress, and
//! hands over to the swap that relaunches the app. Answers come back as
//! actions through `poll`, then `wake`. There is none where the install
//! cannot replace itself (a `cargo run` binary, a `.deb`, Windows for now)
//! or with `SUPERFIK_UPDATES=off`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use superfik_backend::Wake;
use superfik_update::{HttpFetch, HttpReleases, Offer, Platform, Prepared, Stage, UPDATE_KEY, Updater, Version};

use crate::state::{Action, UpdateAction, UpdateCall, UpdateOffer, UpdateStageReport};

const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);

pub struct UpdateRunner {
    updater: Arc<Updater>,
    offer: Arc<Mutex<Option<Offer>>>,
    prepared: Arc<Mutex<Option<Prepared>>>,
    sender: crossbeam_channel::Sender<Action>,
    actions: crossbeam_channel::Receiver<Action>,
    wake: Wake,
    quitting: Arc<AtomicBool>,
}

impl std::fmt::Debug for UpdateRunner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UpdateRunner").finish_non_exhaustive()
    }
}

pub fn offer_of(offer: &Offer) -> UpdateOffer {
    UpdateOffer { version: offer.version.to_string(), required: offer.required }
}

pub fn report(stage: Stage, last: &mut Option<u16>) -> Option<UpdateStageReport> {
    let report = match stage {
        Stage::Checking => UpdateStageReport::Checking,
        Stage::Starting => UpdateStageReport::Starting,
        Stage::Downloading { done, total } => {
            let per_mille = done.saturating_mul(1000).checked_div(total).map_or(0, |share| share.min(1000) as u16);
            if last.is_some_and(|before| per_mille < before.saturating_add(5) && per_mille != 1000) {
                return None;
            }
            *last = Some(per_mille);
            UpdateStageReport::Downloading { per_mille }
        }
        Stage::Verifying => UpdateStageReport::Verifying,
        Stage::Installing => UpdateStageReport::Installing,
    };
    Some(report)
}

impl UpdateRunner {
    pub fn detect(wake: Wake) -> Option<Self> {
        if std::env::var("SUPERFIK_UPDATES").is_ok_and(|value| value == "off") {
            return None;
        }
        let platform = Platform::current()?;
        let installer = superfik_update::installer()?;
        let cache = superfik_backend::local_data_dir().unwrap_or_else(std::env::temp_dir).join("updates");
        let updater = Updater::new(
            Box::new(HttpReleases::from_env()),
            Box::new(HttpFetch),
            installer,
            UPDATE_KEY,
            cache,
            Version::current(),
            platform,
        );
        let (sender, actions) = crossbeam_channel::unbounded();
        let runner = Self {
            updater: Arc::new(updater),
            offer: Arc::default(),
            prepared: Arc::default(),
            sender,
            actions,
            wake,
            quitting: Arc::default(),
        };
        runner.keep_checking();
        Some(runner)
    }

    fn keep_checking(&self) {
        let (updater, offer, sender, wake) =
            (Arc::clone(&self.updater), Arc::clone(&self.offer), self.sender.clone(), self.wake.clone());
        let work = move || {
            loop {
                match updater.check() {
                    Ok(found) => {
                        let shown = found.as_ref().map(offer_of);
                        *offer.lock().unwrap_or_else(PoisonError::into_inner) = found;
                        let _ = sender.send(Action::Update(UpdateAction::Offered(shown)));
                        wake();
                    }
                    Err(error) => tracing::info!(%error, "could not ask for updates"),
                }
                std::thread::sleep(CHECK_EVERY);
            }
        };
        if let Err(error) = std::thread::Builder::new().name("superfik-update-check".to_owned()).spawn(work) {
            tracing::error!(%error, "could not start checking for updates");
        }
    }

    pub fn run(&self, call: UpdateCall) {
        match call {
            UpdateCall::Prepare => self.prepare(),
            UpdateCall::Restart => self.restart(),
        }
    }

    fn prepare(&self) {
        let Some(offer) = self.offer.lock().unwrap_or_else(PoisonError::into_inner).clone() else { return };
        let (updater, prepared, sender, wake) =
            (Arc::clone(&self.updater), Arc::clone(&self.prepared), self.sender.clone(), self.wake.clone());
        let work = move || {
            let mut last = None;
            let mut progress = |stage| {
                if let Some(report) = report(stage, &mut last) {
                    let _ = sender.send(Action::Update(UpdateAction::Stage(report)));
                    wake();
                }
            };
            let answer = match updater.prepare(&offer, &mut progress) {
                Ok(staged) => {
                    *prepared.lock().unwrap_or_else(PoisonError::into_inner) = Some(staged);
                    UpdateAction::Ready
                }
                Err(error) => {
                    tracing::warn!(%error, "the update did not finish");
                    UpdateAction::Failed(error.to_string())
                }
            };
            let _ = sender.send(Action::Update(answer));
            wake();
        };
        if let Err(error) = std::thread::Builder::new().name("superfik-update".to_owned()).spawn(work) {
            tracing::error!(%error, "could not start the update");
        }
    }

    fn restart(&self) {
        let Some(staged) = self.prepared.lock().unwrap_or_else(PoisonError::into_inner).clone() else { return };
        match self.updater.restart(&staged) {
            Ok(()) => {
                self.quitting.store(true, Ordering::SeqCst);
                (self.wake)();
            }
            Err(error) => {
                tracing::error!(%error, "could not restart into the update");
                let _ = self.sender.send(Action::Update(UpdateAction::Failed(error.to_string())));
                (self.wake)();
            }
        }
    }

    pub fn poll(&self) -> Vec<Action> {
        self.actions.try_iter().collect()
    }

    pub fn quitting(&self) -> bool {
        self.quitting.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_download_is_reported_as_it_moves_on_and_every_other_stage_once() {
        let mut last = None;
        let at = |done| Stage::Downloading { done, total: 1000 };
        assert_eq!(report(Stage::Starting, &mut last), Some(UpdateStageReport::Starting));
        assert_eq!(report(at(0), &mut last), Some(UpdateStageReport::Downloading { per_mille: 0 }));
        assert_eq!(report(at(3), &mut last), None, "half a percent at least");
        assert_eq!(report(at(6), &mut last), Some(UpdateStageReport::Downloading { per_mille: 6 }));
        assert_eq!(report(at(1000), &mut last), Some(UpdateStageReport::Downloading { per_mille: 1000 }));
        assert_eq!(report(Stage::Verifying, &mut last), Some(UpdateStageReport::Verifying));
        let unknown = Stage::Downloading { done: 10, total: 0 };
        assert_eq!(report(unknown, &mut None), Some(UpdateStageReport::Downloading { per_mille: 0 }));
    }

    #[test]
    fn an_offer_reads_as_its_version_and_whether_it_is_required() {
        let asset = superfik_update::Asset {
            url: "https://x".into(),
            size: 1,
            sha256: String::new(),
            signature: String::new(),
        };
        let offer = Offer { version: Version::new(0, 2, 0), required: true, asset, notes_url: None };
        assert_eq!(offer_of(&offer), UpdateOffer { version: "0.2.0".into(), required: true });
    }
}
