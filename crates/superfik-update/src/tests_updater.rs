//! `Updater` end to end over stand-ins.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};

use crate::*;

const ARCHIVE: &[u8] = b"the new Superfik, zipped";

struct Latest(Option<Manifest>);

impl Releases for Latest {
    fn latest(&self, _: Platform, _: &Version) -> Result<Option<Manifest>, UpdateError> {
        Ok(self.0.clone())
    }
}

struct Serve(Vec<u8>);

impl Fetch for Serve {
    fn download(&self, _: &str, to: &Path, progress: &mut dyn FnMut(u64, u64)) -> Result<(), UpdateError> {
        progress(0, 0);
        std::fs::write(to, &self.0).map_err(|e| UpdateError::Install(e.to_string()))?;
        progress(self.0.len() as u64, 0);
        Ok(())
    }
}

type Staged = (PathBuf, Version, Vec<u8>);

#[derive(Clone, Default)]
struct Recorder(Arc<Mutex<Vec<Staged>>>);

impl Installer for Recorder {
    fn stage(&self, archive: &Path, version: &Version) -> Result<Prepared, UpdateError> {
        let bytes = std::fs::read(archive).unwrap_or_default();
        self.0.lock().expect("lock").push((archive.to_path_buf(), *version, bytes));
        Ok(Prepared { version: *version, staged: PathBuf::from("/staged/Superfik.app") })
    }

    fn restart(&self, _: &Prepared) -> Result<(), UpdateError> {
        Ok(())
    }
}

fn key() -> SigningKey {
    SigningKey::from_bytes(&[42; 32])
}

fn asset_for(bytes: &[u8], signer: &SigningKey) -> Asset {
    let digest: [u8; 32] = Sha256::digest(bytes).into();
    Asset {
        url: "https://example.com/Superfik-0.15.0-macos-aarch64.zip".into(),
        size: bytes.len() as u64,
        sha256: hex::encode(digest),
        signature: hex::encode(signer.sign(&digest).to_bytes()),
    }
}

fn manifest(asset: Asset) -> Manifest {
    Manifest { version: Version::new(0, 15, 0), minimum: None, notes_url: None, published_unix: 1, asset }
}

fn updater(asset: Asset, served: &[u8], recorder: &Recorder, cache: &Path) -> Updater {
    Updater::new(
        Box::new(Latest(Some(manifest(asset)))),
        Box::new(Serve(served.to_vec())),
        Box::new(recorder.clone()),
        key().verifying_key().to_bytes(),
        cache.to_path_buf(),
        Version::new(0, 14, 0),
        Platform::MacosAarch64,
    )
}

fn offered(updater: &Updater) -> Offer {
    updater.check().expect("check").expect("an offer")
}

#[test]
fn checks_downloads_verifies_and_stages() {
    let cache = tempfile::tempdir().expect("temp dir");
    let recorder = Recorder::default();
    let updater = updater(asset_for(ARCHIVE, &key()), ARCHIVE, &recorder, cache.path());
    let offer = offered(&updater);
    assert_eq!((offer.version, offer.required), (Version::new(0, 15, 0), false));
    let mut stages = Vec::new();
    let prepared = updater.prepare(&offer, &mut |stage| stages.push(stage)).expect("prepared");
    assert_eq!(prepared.version, Version::new(0, 15, 0));
    let total = ARCHIVE.len() as u64;
    assert_eq!(
        stages,
        [
            Stage::Starting,
            Stage::Downloading { done: 0, total },
            Stage::Downloading { done: total, total },
            Stage::Verifying,
            Stage::Installing,
        ]
    );
    let staged = recorder.0.lock().expect("lock").clone();
    assert_eq!(staged.len(), 1);
    assert_eq!((staged[0].1, staged[0].2.as_slice()), (Version::new(0, 15, 0), ARCHIVE));
    assert!(staged[0].0.starts_with(cache.path()));
    assert!(!staged[0].0.exists(), "the download is removed once staged");
}

#[test]
fn nothing_is_offered_when_up_to_date_or_unreleased() {
    let cache = tempfile::tempdir().expect("temp dir");
    let make = |releases: Latest, current| {
        Updater::new(
            Box::new(releases),
            Box::new(Serve(Vec::new())),
            Box::new(Recorder::default()),
            [0; 32],
            cache.path().to_path_buf(),
            current,
            Platform::LinuxX86_64,
        )
    };
    let asset = asset_for(ARCHIVE, &key());
    assert_eq!(make(Latest(Some(manifest(asset))), Version::new(0, 15, 0)).check(), Ok(None));
    assert_eq!(make(Latest(None), Version::new(0, 1, 0)).check(), Ok(None));
}

#[test]
fn a_different_size_is_corrupt() {
    let cache = tempfile::tempdir().expect("temp dir");
    let recorder = Recorder::default();
    let mut longer = ARCHIVE.to_vec();
    longer.push(b'!');
    let updater = updater(asset_for(ARCHIVE, &key()), &longer, &recorder, cache.path());
    assert_eq!(updater.prepare(&offered(&updater), &mut |_| {}), Err(UpdateError::Corrupt));
    assert!(recorder.0.lock().expect("lock").is_empty());
}

#[test]
fn different_bytes_of_the_same_size_are_corrupt() {
    let cache = tempfile::tempdir().expect("temp dir");
    let recorder = Recorder::default();
    let mut tampered = ARCHIVE.to_vec();
    tampered[0] ^= 1;
    let updater = updater(asset_for(ARCHIVE, &key()), &tampered, &recorder, cache.path());
    assert_eq!(updater.prepare(&offered(&updater), &mut |_| {}), Err(UpdateError::Corrupt));
    assert!(recorder.0.lock().expect("lock").is_empty());
    assert_eq!(std::fs::read_dir(cache.path()).map(Iterator::count).ok(), Some(0), "nothing left behind");
}

#[test]
fn a_signature_by_another_key_is_forged() {
    let cache = tempfile::tempdir().expect("temp dir");
    let recorder = Recorder::default();
    let forger = SigningKey::from_bytes(&[13; 32]);
    let updater = updater(asset_for(ARCHIVE, &forger), ARCHIVE, &recorder, cache.path());
    let mut stages = Vec::new();
    assert_eq!(updater.prepare(&offered(&updater), &mut |s| stages.push(s)), Err(UpdateError::Forged));
    assert!(!stages.contains(&Stage::Installing));
    assert!(recorder.0.lock().expect("lock").is_empty());
}
