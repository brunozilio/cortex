use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use domain::legacy::OWNER_DIR;
use domain::trust::{Refusal, Trust, TrustReport, TrustedFolders};
use platform::store::Document;
use platform::sync::Locked;

pub const FILE: &str = "trusted-folders.json";

type Change<'a> = &'a dyn Fn(&mut TrustedFolders) -> Result<(), Refusal>;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TrustError {
    #[error("{0}")]
    Refused(Refusal),
    #[error("{0}")]
    Unsaved(String),
}

impl From<Refusal> for TrustError {
    fn from(refusal: Refusal) -> Self {
        Self::Refused(refusal)
    }
}

impl From<TrustError> for String {
    fn from(error: TrustError) -> Self {
        error.to_string()
    }
}

pub trait TrustStore: Send + Sync {
    fn folders(&self) -> TrustedFolders;
    fn change(&self, change: Change<'_>) -> Result<(), TrustError>;
}

#[derive(Debug, Clone)]
pub struct FileTrustStore {
    kept: Document,
}

impl FileTrustStore {
    pub fn new(users: PathBuf) -> Self {
        Self { kept: Document::at(users.join(OWNER_DIR).join(FILE)).compact() }
    }
}

impl TrustStore for FileTrustStore {
    fn folders(&self) -> TrustedFolders {
        match self.kept.read() {
            Ok(folders) => folders.unwrap_or_default(),
            Err(error) => {
                tracing::warn!(%error, "the trusted folders could not be read; no folder is trusted");
                TrustedFolders::default()
            }
        }
    }

    fn change(&self, change: Change<'_>) -> Result<(), TrustError> {
        let changed = self.kept.try_update(|folders: &mut TrustedFolders| change(folders));
        Ok(changed.map_err(|error| TrustError::Unsaved(error.to_string()))??)
    }
}

#[derive(Debug, Default)]
pub struct InMemoryTrustStore {
    kept: Mutex<TrustedFolders>,
}

impl TrustStore for InMemoryTrustStore {
    fn folders(&self) -> TrustedFolders {
        self.kept.locked().clone()
    }

    fn change(&self, change: Change<'_>) -> Result<(), TrustError> {
        Ok(change(&mut self.kept.locked())?)
    }
}

#[derive(Clone)]
pub struct Trusts {
    store: Arc<dyn TrustStore>,
    for_this_process: Arc<Mutex<Vec<PathBuf>>>,
}

impl Default for Trusts {
    fn default() -> Self {
        Self::kept(Arc::new(InMemoryTrustStore::default()))
    }
}

impl std::fmt::Debug for Trusts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Trusts").finish_non_exhaustive()
    }
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|since| since.as_secs()).unwrap_or_default()
}

impl Trusts {
    pub fn kept(store: Arc<dyn TrustStore>) -> Self {
        Self { store, for_this_process: Arc::default() }
    }

    pub fn of(&self, cwd: &Path, home: Option<&Path>) -> Trust {
        let root = agent_engine::trust::root_of(cwd, home);
        if self.for_this_process.locked().contains(&root) {
            return Trust::Trusted;
        }
        self.store.folders().of_root(&root)
    }

    pub fn stored(&self) -> TrustedFolders {
        self.store.folders()
    }

    pub fn gate(&self, cwd: &Path, home: Option<&Path>) -> agent_engine::trust::Gate {
        agent_engine::trust::Gate::at(cwd, home, self.of(cwd, home))
    }

    pub fn report(&self, cwd: &Path, home: Option<&Path>) -> TrustReport {
        agent_engine::trust::report(cwd, home, self.of(cwd, home))
    }

    pub fn set(&self, cwd: &Path, home: Option<&Path>, trusted: bool) -> Result<TrustReport, TrustError> {
        let root = agent_engine::trust::root_of(cwd, home);
        let home_folder = home.map(agent_engine::trust::canonical);
        if trusted {
            self.store.change(&|folders| folders.trust(&root, home_folder.as_deref(), now()))?;
        } else {
            self.for_this_process.locked().retain(|known| *known != root);
            self.store.change(&|folders| {
                folders.untrust(&root);
                Ok(())
            })?;
        }
        Ok(self.report(cwd, home))
    }

    pub fn only_for_this_process(&self, cwd: &Path, home: Option<&Path>) -> Result<TrustReport, TrustError> {
        let root = agent_engine::trust::root_of(cwd, home);
        if let Some(refusal) = domain::trust::refusal(&root, home.map(agent_engine::trust::canonical).as_deref()) {
            return Err(TrustError::Refused(refusal));
        }
        let mut trusted = self.for_this_process.locked();
        if !trusted.contains(&root) {
            trusted.push(root);
        }
        drop(trusted);
        Ok(self.report(cwd, home))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Asked {
    Kept(bool),
    ForThisProcess,
    State,
}

pub(crate) fn answer(services: &crate::LocalServices, path: &Path, asked: Asked) -> crate::api::Reply {
    let home = services.home();
    let (home, trusts) = (home.as_deref(), &services.extensions.trusts);
    let changed = match asked {
        Asked::State => return crate::api::Reply::Trust(trusts.report(path, home)),
        Asked::Kept(trusted) => trusts.set(path, home, trusted),
        Asked::ForThisProcess => trusts.only_for_this_process(path, home),
    };
    match changed {
        Ok(report) => {
            services.follow_project(path);
            crate::api::Reply::Trust(report)
        }
        Err(refused) => crate::api::Reply::error(refused.to_string()),
    }
}

#[cfg(test)]
#[path = "trust_tests.rs"]
mod tests;
