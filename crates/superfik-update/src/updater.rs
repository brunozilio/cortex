//! An update from start to finish: ask the service, download the archive,
//! check it, stage it, and hand over to the swap.

use std::path::PathBuf;

use crate::verify::{digest_file, matches};
use crate::{Fetch, Installer, Offer, Platform, Prepared, Releases, Stage, UpdateError, Version, offer, verify};

pub struct Updater {
    releases: Box<dyn Releases>,
    fetch: Box<dyn Fetch>,
    installer: Box<dyn Installer>,
    key: [u8; 32],
    cache: PathBuf,
    current: Version,
    platform: Platform,
}

impl std::fmt::Debug for Updater {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Updater")
            .field("cache", &self.cache)
            .field("current", &self.current)
            .field("platform", &self.platform)
            .finish()
    }
}

impl Updater {
    pub fn new(
        releases: Box<dyn Releases>,
        fetch: Box<dyn Fetch>,
        installer: Box<dyn Installer>,
        key: [u8; 32],
        cache: PathBuf,
        current: Version,
        platform: Platform,
    ) -> Self {
        Self { releases, fetch, installer, key, cache, current, platform }
    }

    pub fn check(&self) -> Result<Option<Offer>, UpdateError> {
        let manifest = self.releases.latest(self.platform, &self.current)?;
        Ok(manifest.and_then(|manifest| offer(&self.current, &manifest)))
    }

    pub fn prepare(&self, offer: &Offer, progress: &mut dyn FnMut(Stage)) -> Result<Prepared, UpdateError> {
        progress(Stage::Starting);
        std::fs::create_dir_all(&self.cache).map_err(UpdateError::install)?;
        let archive = self.cache.join(format!("superfik-{}-{}.download", offer.version, self.platform.as_str()));
        let prepared = self.fetch_and_stage(offer, &archive, progress);
        let _ = std::fs::remove_file(&archive);
        prepared
    }

    fn fetch_and_stage(
        &self,
        offer: &Offer,
        archive: &std::path::Path,
        progress: &mut dyn FnMut(Stage),
    ) -> Result<Prepared, UpdateError> {
        let expected = offer.asset.size;
        self.fetch.download(&offer.asset.url, archive, &mut |done, total| {
            progress(Stage::Downloading { done, total: if total == 0 { expected } else { total } });
        })?;
        progress(Stage::Verifying);
        let (size, digest) = digest_file(archive).map_err(UpdateError::install)?;
        if !matches(&offer.asset, size, &digest) {
            return Err(UpdateError::Corrupt);
        }
        verify(&digest, &offer.asset, &self.key)?;
        progress(Stage::Installing);
        self.installer.stage(archive, &offer.version)
    }

    pub fn restart(&self, prepared: &Prepared) -> Result<(), UpdateError> {
        self.installer.restart(prepared)
    }
}
