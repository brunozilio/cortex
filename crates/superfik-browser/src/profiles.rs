//! The local profile library, separate from CEF-owned conversation directories.

mod credentials;
pub use credentials::{CredentialStore, KeychainCredentials, SecretChange};

use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use superfik_core::browser_profiles::{ProfileBook, ProfileId};

#[derive(Debug)]
pub struct ProfileStore {
    root: PathBuf,
}

impl ProfileStore {
    pub fn save_with_credentials(
        &self,
        book: &ProfileBook,
        change: Option<&SecretChange>,
        credentials: &dyn CredentialStore,
    ) -> Result<(), String> {
        book.validate()?;
        let existing = self.load()?;
        for profile in &book.profiles {
            let authenticated = profile.settings.proxy.as_ref().is_some_and(|p| p.username.is_some());
            let changed = existing.profiles.iter().find(|p| p.id == profile.id) != Some(profile);
            if authenticated && changed {
                let password = match change.filter(|c| c.id == profile.id) {
                    Some(change) => change.password.clone(),
                    None => credentials.read(profile.id)?,
                };
                if password.is_none() {
                    return Err("A proxy password is required; it will be stored in Keychain".into());
                }
            }
        }
        let previous = if let Some(change) = change {
            let previous = credentials.read(change.id)?;
            credentials.write(change.id, change.password.as_deref())?;
            previous
        } else {
            None
        };
        if let Err(error) = self.save(book) {
            if let Some(change) = change {
                credentials
                    .write(change.id, previous.as_deref())
                    .map_err(|rollback| format!("{error}; restoring proxy credential failed: {rollback}"))?;
            }
            return Err(error);
        }
        Ok(())
    }
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
    pub fn path(&self) -> PathBuf {
        self.root.join("superfik-profiles.json")
    }

    pub fn load(&self) -> Result<ProfileBook, String> {
        let file = match fs::File::open(self.path()) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(ProfileBook::default()),
            Err(e) => return Err(e.to_string()),
        };
        let mut json = String::new();
        file.take(1024 * 1024 + 1).read_to_string(&mut json).map_err(|e| e.to_string())?;
        ProfileBook::parse(&json)
    }

    pub fn save(&self, book: &ProfileBook) -> Result<(), String> {
        book.validate()?;
        let json = serde_json::to_vec_pretty(book).map_err(|e| e.to_string())?;
        if json.len() > 1024 * 1024 {
            return Err("Profile library exceeds 1 MiB".into());
        }
        fs::create_dir_all(&self.root).map_err(|e| e.to_string())?;
        let temporary = self.root.join(format!(".superfik-profiles-{}.tmp", ProfileId::new()));
        let result = self.replace(&temporary, &json);
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result.map_err(|e| e.to_string())
    }

    fn replace(&self, temporary: &Path, json: &[u8]) -> std::io::Result<()> {
        let mut file = superfik_private_file::open(OpenOptions::new().write(true).create_new(true), temporary)?;
        file.write_all(json)?;
        file.sync_all()?;
        fs::rename(temporary, self.path())
    }
}
