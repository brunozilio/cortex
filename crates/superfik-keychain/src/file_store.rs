//! Credentials in a file only the user can read (`0600`), for a Linux
//! desktop whose Secret Service has no keyring to keep them in (a session
//! signed in automatically never gets GNOME's "Login" one, and creating it
//! asks for a password). Chromium does the same with `password-store=basic`.
//! The file is a JSON object of `service/account` → value; every write
//! replaces it whole ([`superfik_private_file::replace`]).

use std::any::Any;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use keyring::credential::{Credential, CredentialApi, CredentialBuilderApi, CredentialPersistence};

pub const FILE_NAME: &str = "credentials.json";

pub fn default_path() -> Option<PathBuf> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(config.join("superfik-lite").join(FILE_NAME))
}

#[derive(Debug, Clone)]
pub struct FileStore {
    path: PathBuf,
    turn: Arc<Mutex<()>>,
}

impl FileStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path, turn: Arc::new(Mutex::new(())) }
    }
}

impl CredentialBuilderApi for FileStore {
    fn build(&self, _target: Option<&str>, service: &str, user: &str) -> keyring::Result<Box<Credential>> {
        Ok(Box::new(FileCredential { store: self.clone(), key: format!("{service}/{user}") }))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn persistence(&self) -> CredentialPersistence {
        CredentialPersistence::UntilDelete
    }
}

#[derive(Debug)]
struct FileCredential {
    store: FileStore,
    key: String,
}

impl FileCredential {
    fn change(&self, edit: impl FnOnce(&mut BTreeMap<String, String>) -> bool) -> keyring::Result<()> {
        let _turn = self.store.turn.lock().unwrap_or_else(PoisonError::into_inner);
        let mut values = read(&self.store.path)?;
        if !edit(&mut values) {
            return Err(keyring::Error::NoEntry);
        }
        let bytes = serde_json::to_vec_pretty(&values).map_err(|error| failure(error.to_string()))?;
        superfik_private_file::replace(&self.store.path, &bytes).map_err(|error| failure(error.to_string()))
    }
}

impl CredentialApi for FileCredential {
    fn set_secret(&self, secret: &[u8]) -> keyring::Result<()> {
        let value =
            String::from_utf8(secret.to_vec()).map_err(|error| keyring::Error::BadEncoding(error.into_bytes()))?;
        self.change(|values| {
            values.insert(self.key.clone(), value);
            true
        })
    }

    fn get_secret(&self) -> keyring::Result<Vec<u8>> {
        let _turn = self.store.turn.lock().unwrap_or_else(PoisonError::into_inner);
        read(&self.store.path)?.remove(&self.key).map(String::into_bytes).ok_or(keyring::Error::NoEntry)
    }

    fn delete_credential(&self) -> keyring::Result<()> {
        self.change(|values| values.remove(&self.key).is_some())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn read(path: &Path) -> keyring::Result<BTreeMap<String, String>> {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|error| failure(format!("{}: {error}", path.display()))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(error) => Err(failure(error.to_string())),
    }
}

fn failure(message: String) -> keyring::Error {
    keyring::Error::PlatformFailure(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(store: &FileStore, account: &str) -> keyring::Entry {
        keyring::Entry::new_with_credential(store.build(None, "Superfik", account).expect("credential"))
    }

    fn store() -> (tempfile::TempDir, FileStore) {
        let dir = tempfile::tempdir().expect("temporary folder");
        let store = FileStore::new(dir.path().join("superfik").join(FILE_NAME));
        (dir, store)
    }

    #[test]
    fn a_kept_value_reads_back_and_survives_a_new_store() {
        let (_dir, store) = store();
        entry(&store, "refresh-token").set_password("token").expect("kept");
        let again = FileStore::new(store.path.clone());
        assert_eq!(entry(&again, "refresh-token").get_password().expect("read"), "token");
    }

    #[test]
    fn values_stay_apart_and_a_missing_one_is_no_entry() {
        let (_dir, store) = store();
        entry(&store, "a").set_password("1").expect("kept");
        entry(&store, "b").set_password("2").expect("kept");
        assert_eq!(entry(&store, "a").get_password().expect("read"), "1");
        assert!(matches!(entry(&store, "c").get_password(), Err(keyring::Error::NoEntry)));
    }

    #[test]
    fn deleting_forgets_the_value_only() {
        let (_dir, store) = store();
        entry(&store, "a").set_password("1").expect("kept");
        entry(&store, "b").set_password("2").expect("kept");
        entry(&store, "a").delete_credential().expect("deleted");
        assert!(matches!(entry(&store, "a").get_password(), Err(keyring::Error::NoEntry)));
        assert!(matches!(entry(&store, "a").delete_credential(), Err(keyring::Error::NoEntry)));
        assert_eq!(entry(&store, "b").get_password().expect("read"), "2");
    }

    #[cfg(unix)]
    #[test]
    fn only_the_user_can_read_the_file() {
        use std::os::unix::fs::PermissionsExt;
        let (_dir, store) = store();
        entry(&store, "a").set_password("1").expect("kept");
        let mode = std::fs::metadata(&store.path).expect("file").permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
