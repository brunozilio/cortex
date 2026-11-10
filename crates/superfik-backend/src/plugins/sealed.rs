//! Connection secrets sealed in one file, for servers without a system
//! keychain (a Linux container's keyring does not survive a restart).
//! Each connection's secrets are encrypted on their own with
//! XChaCha20-Poly1305: a fresh random nonce each time, and the
//! connection's key as associated data, so an entry moved to another
//! connection fails to open. The key is 32 bytes kept apart from the file
//! (`secrets.key`, `0600`), or handed in (`SUPERFIK_SECRET_KEY`, e.g. from
//! a KMS).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use serde::{Deserialize, Serialize};
use superfik_core::{ConnectionId, Principal};

use super::secrets::{SecretStore, Secrets};
use crate::memory::owner_key;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Sealed {
    nonce: String,
    sealed: String,
}

pub struct SealedSecretStore {
    path: PathBuf,
    cipher: XChaCha20Poly1305,
    entries: Mutex<BTreeMap<String, Sealed>>,
}

impl std::fmt::Debug for SealedSecretStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SealedSecretStore").field("path", &self.path).finish_non_exhaustive()
    }
}

impl SealedSecretStore {
    pub fn open(path: &Path, key: [u8; 32]) -> Result<Self, String> {
        let entries = match std::fs::read_to_string(path) {
            Ok(json) => serde_json::from_str(&json).map_err(|error| error.to_string())?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
            Err(error) => return Err(error.to_string()),
        };
        let cipher = XChaCha20Poly1305::new(&key.into());
        Ok(Self { path: path.to_path_buf(), cipher, entries: Mutex::new(entries) })
    }

    pub fn key_at(path: &Path) -> Result<[u8; 32], String> {
        if let Ok(text) = std::fs::read_to_string(path) {
            return unhex(text.trim())
                .and_then(|bytes| bytes.try_into().ok())
                .ok_or("the secrets key is not 64 hex digits".into());
        }
        let mut key = [0u8; 32];
        getrandom::fill(&mut key).map_err(|error| error.to_string())?;
        write_private(path, &hex(&key)).map_err(|error| error.to_string())?;
        Ok(key)
    }

    /// The entries as the file holds them now (another process, such as a
    /// terminal running without the daemon, may have written since).
    fn entries(&self) -> MutexGuard<'_, BTreeMap<String, Sealed>> {
        let mut entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        if let Ok(json) = std::fs::read_to_string(&self.path)
            && let Ok(fresh) = serde_json::from_str(&json)
        {
            *entries = fresh;
        }
        entries
    }

    fn save(&self, entries: &BTreeMap<String, Sealed>) -> Result<(), String> {
        let json = serde_json::to_string_pretty(entries).map_err(|error| error.to_string())?;
        write_private(&self.path, &json).map_err(|error| error.to_string())
    }
}

fn name(owner: &Principal, connection: ConnectionId) -> String {
    format!("{}/{connection}", owner_key(owner))
}

impl SecretStore for SealedSecretStore {
    fn get(&self, owner: &Principal, connection: ConnectionId) -> Option<Secrets> {
        let name = name(owner, connection);
        let entry = self.entries().get(&name)?.clone();
        let nonce: [u8; 24] = unhex(&entry.nonce)?.try_into().ok()?;
        let opened = self
            .cipher
            .decrypt(XNonce::from_slice(&nonce), Payload { msg: &unhex(&entry.sealed)?, aad: name.as_bytes() })
            .ok()?;
        serde_json::from_slice(&opened).ok()
    }

    fn set(&self, owner: &Principal, connection: ConnectionId, secrets: &Secrets) -> Result<(), String> {
        let name = name(owner, connection);
        let json = serde_json::to_vec(secrets).map_err(|error| error.to_string())?;
        let mut nonce = [0u8; 24];
        getrandom::fill(&mut nonce).map_err(|error| error.to_string())?;
        let sealed = self
            .cipher
            .encrypt(XNonce::from_slice(&nonce), Payload { msg: &json, aad: name.as_bytes() })
            .map_err(|_| "could not seal the secrets".to_owned())?;
        let mut entries = self.entries();
        entries.insert(name, Sealed { nonce: hex(&nonce), sealed: hex(&sealed) });
        self.save(&entries)
    }

    fn delete(&self, owner: &Principal, connection: ConnectionId) {
        let mut entries = self.entries();
        if entries.remove(&name(owner, connection)).is_some()
            && let Err(error) = self.save(&entries)
        {
            tracing::error!(%error, "could not forget the secrets");
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    (0..text.len())
        .step_by(2)
        .map(|at| text.get(at..at + 2).and_then(|pair| u8::from_str_radix(pair, 16).ok()))
        .collect()
}

fn write_private(path: &Path, text: &str) -> std::io::Result<()> {
    superfik_private_file::replace(path, text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_processes_on_one_file_keep_each_others_secrets() {
        let dir = tempfile::tempdir().expect("dir");
        let key = SealedSecretStore::key_at(&dir.path().join("secrets.key")).expect("key");
        let file = dir.path().join("secrets.json");
        let (one, two) =
            (SealedSecretStore::open(&file, key).expect("one"), SealedSecretStore::open(&file, key).expect("two"));
        let owner = Principal::LocalOwner;
        let (a, b) = (ConnectionId::new(), ConnectionId::new());
        one.set(&owner, a, &Secrets::from([("k".into(), "from one".into())])).expect("set a");
        two.set(&owner, b, &Secrets::from([("k".into(), "from two".into())])).expect("set b");
        assert_eq!(two.get(&owner, a).and_then(|s| s.get("k").cloned()).as_deref(), Some("from one"));
        assert_eq!(one.get(&owner, b).and_then(|s| s.get("k").cloned()).as_deref(), Some("from two"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&file).expect("meta").permissions().mode() & 0o777, 0o600);
        }
    }

    fn secrets() -> Secrets {
        Secrets::from([("api_key".to_owned(), "lin_api_very_secret".to_owned())])
    }

    #[test]
    fn secrets_survive_reopening_and_never_sit_in_the_clear() {
        let dir = tempfile::tempdir().expect("dir");
        let (path, key) =
            (dir.path().join("secrets.json"), SealedSecretStore::key_at(&dir.path().join("secrets.key")).expect("key"));
        let connection = ConnectionId::new();
        SealedSecretStore::open(&path, key)
            .expect("open")
            .set(&Principal::LocalOwner, connection, &secrets())
            .expect("set");
        let reopened = SealedSecretStore::open(&path, key).expect("reopen");
        assert_eq!(reopened.get(&Principal::LocalOwner, connection), Some(secrets()));
        assert!(!std::fs::read_to_string(&path).expect("read").contains("very_secret"));
    }

    #[test]
    fn another_key_opens_nothing_and_the_key_is_kept() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("secrets.json");
        let key_path = dir.path().join("secrets.key");
        let key = SealedSecretStore::key_at(&key_path).expect("key");
        assert_eq!(SealedSecretStore::key_at(&key_path).expect("again"), key, "made once, then read");
        let connection = ConnectionId::new();
        SealedSecretStore::open(&path, key)
            .expect("open")
            .set(&Principal::LocalOwner, connection, &secrets())
            .expect("set");
        let stranger = SealedSecretStore::open(&path, [9; 32]).expect("open");
        assert_eq!(stranger.get(&Principal::LocalOwner, connection), None);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            for file in [&path, &key_path] {
                assert_eq!(std::fs::metadata(file).expect("meta").permissions().mode() & 0o777, 0o600);
            }
        }
    }

    #[test]
    fn an_entry_moved_to_another_connection_does_not_open() {
        let dir = tempfile::tempdir().expect("dir");
        let store = SealedSecretStore::open(&dir.path().join("secrets.json"), [3; 32]).expect("open");
        let (a, b) = (ConnectionId::new(), ConnectionId::new());
        store.set(&Principal::LocalOwner, a, &secrets()).expect("set");
        let moved = store.entries().get(&name(&Principal::LocalOwner, a)).cloned().expect("sealed");
        store.entries().insert(name(&Principal::LocalOwner, b), moved);
        assert_eq!(store.get(&Principal::LocalOwner, b), None);
    }

    #[test]
    fn forgetting_removes_the_entry() {
        let dir = tempfile::tempdir().expect("dir");
        let store = SealedSecretStore::open(&dir.path().join("secrets.json"), [3; 32]).expect("open");
        let connection = ConnectionId::new();
        store.set(&Principal::LocalOwner, connection, &secrets()).expect("set");
        store.delete(&Principal::LocalOwner, connection);
        assert_eq!(store.get(&Principal::LocalOwner, connection), None);
    }
}
