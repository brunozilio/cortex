use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;

use platform::file_lock::{Exclusive, Kept};
use platform::store::{Document, Error};
use platform::sync::Locked;

const REFRESH_WAIT: std::time::Duration = std::time::Duration::from_secs(60);

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use domain::SecretId;
use domain::legacy::OWNER_DIR;
use serde::{Deserialize, Serialize};

use super::{SecretStore, Secrets};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Sealed {
    nonce: String,
    sealed: String,
}

type Entries = BTreeMap<String, Sealed>;

pub struct SealedSecretStore {
    vault: Document,
    cipher: XChaCha20Poly1305,
    unreadable: Mutex<Option<String>>,
}

impl std::fmt::Debug for SealedSecretStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SealedSecretStore").field("path", &self.vault.path()).finish_non_exhaustive()
    }
}

impl SealedSecretStore {
    pub fn open(path: &Path, key: [u8; 32]) -> Result<Self, String> {
        let vault = Document::at(path);
        let entries = match vault.read::<Entries>() {
            Ok(entries) => entries.unwrap_or_default(),
            Err(error @ Error::Corrupt { .. }) => {
                tracing::warn!(%error, "the secrets file did not parse; kept aside");
                Entries::new()
            }
            Err(error) => return Err(format!("the secrets file cannot be used: {error}")),
        };
        let store = Self { vault, cipher: XChaCha20Poly1305::new(&key.into()), unreadable: Mutex::new(None) };
        let closed = entries.iter().filter(|(name, entry)| store.opened(name, entry).is_none()).count();
        if closed > 0 {
            store.found_unreadable(closed);
        }
        Ok(store)
    }

    fn found_unreadable(&self, count: usize) -> String {
        let reason = format!(
            "{count} saved secret{} in {} cannot be decrypted with this machine's key (was the key replaced?); they \
             were left as they are. Restore the key that sealed them, or move that file aside to start over",
            if count == 1 { "" } else { "s" },
            self.vault.path().display()
        );
        tracing::error!(%reason, "the secrets vault holds entries this key cannot open");
        self.unreadable.locked().replace(reason.clone());
        reason
    }

    fn opened_or_refused(&self, name: &str, entry: Option<&Sealed>) -> Result<Option<Secrets>, String> {
        let Some(entry) = entry else { return Ok(None) };
        self.opened(name, entry).map(Some).ok_or_else(|| self.found_unreadable(1))
    }

    fn sealed(&self, name: &str, secrets: &Secrets) -> Result<Sealed, String> {
        let json = serde_json::to_vec(secrets).map_err(|error| error.to_string())?;
        let mut nonce = [0u8; 24];
        getrandom::fill(&mut nonce).map_err(|error| error.to_string())?;
        let sealed = self
            .cipher
            .encrypt(XNonce::from_slice(&nonce), Payload { msg: &json, aad: name.as_bytes() })
            .map_err(|_| "could not seal the secrets".to_owned())?;
        Ok(Sealed { nonce: hex(&nonce), sealed: hex(&sealed) })
    }

    fn opened(&self, name: &str, entry: &Sealed) -> Option<Secrets> {
        let nonce: [u8; 24] = unhex(&entry.nonce)?.try_into().ok()?;
        let opened = self
            .cipher
            .decrypt(XNonce::from_slice(&nonce), Payload { msg: &unhex(&entry.sealed)?, aad: name.as_bytes() })
            .ok()?;
        serde_json::from_slice(&opened).ok()
    }

    #[cfg(test)]
    fn key_at(path: &Path) -> Result<[u8; 32], String> {
        super::key::at(path, super::key::Keeper::File)
    }
}

fn name(connection: SecretId) -> String {
    format!("{OWNER_DIR}/{connection}")
}

impl SecretStore for SealedSecretStore {
    fn get(&self, connection: SecretId) -> Option<Secrets> {
        let name = name(connection);
        let entry = self.vault.read_or_default::<Entries>().remove(&name);
        self.opened_or_refused(&name, entry.as_ref()).ok().flatten()
    }

    fn set(&self, connection: SecretId, secrets: &Secrets) -> Result<(), String> {
        let name = name(connection);
        let sealed = self.sealed(&name, secrets)?;
        let kept = self.vault.try_update(|entries: &mut Entries| {
            self.opened_or_refused(&name, entries.get(&name))?;
            entries.insert(name.clone(), sealed);
            Ok::<(), String>(())
        });
        kept.map_err(|error| error.to_string())?
    }

    fn update(&self, connection: SecretId, change: &mut dyn FnMut(&mut Secrets)) -> Result<(), String> {
        let name = name(connection);
        let kept = self.vault.try_update(|entries: &mut Entries| {
            let mut secrets = self.opened_or_refused(&name, entries.get(&name))?.unwrap_or_default();
            change(&mut secrets);
            entries.insert(name.clone(), self.sealed(&name, &secrets)?);
            Ok::<(), String>(())
        });
        kept.map_err(|error| error.to_string())?
    }

    fn unkept(&self) -> Option<String> {
        self.unreadable.locked().clone()
    }

    fn hold_for_refresh(&self, connection: SecretId) -> Result<Option<domain::provider::RefreshHold>, String> {
        let beside = self.vault.path().with_file_name(format!("refresh-{connection}"));
        let held = Exclusive::within(&beside, REFRESH_WAIT, Kept::OnlyWhileHeld)
            .map_err(|error| format!("the sign-in refresh could not be locked: {error}"))?
            .ok_or_else(|| {
                format!("another process has been refreshing this sign-in for over {} s", REFRESH_WAIT.as_secs())
            })?;
        Ok(Some(Box::new(held)))
    }

    fn delete(&self, connection: SecretId) {
        let forgotten = self.vault.update(|entries: &mut Entries| entries.remove(&name(connection)));
        if let Err(error) = forgotten {
            tracing::error!(%error, "could not forget the secrets");
        }
    }
}

pub(super) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(super) fn unhex(text: &str) -> Option<Vec<u8>> {
    (0..text.len())
        .step_by(2)
        .map(|at| text.get(at..at + 2).and_then(|pair| u8::from_str_radix(pair, 16).ok()))
        .collect()
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
        let (a, b) = (SecretId::new(), SecretId::new());
        one.set(a, &Secrets::from([("k".into(), "from one".into())])).expect("set a");
        two.set(b, &Secrets::from([("k".into(), "from two".into())])).expect("set b");
        assert_eq!(two.get(a).and_then(|s| s.get("k").cloned()).as_deref(), Some("from one"));
        assert_eq!(one.get(b).and_then(|s| s.get("k").cloned()).as_deref(), Some("from two"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&file).expect("meta").permissions().mode() & 0o777, 0o600);
        }
    }

    #[test]
    fn writers_at_once_lose_nothing() {
        let dir = tempfile::tempdir().expect("dir");
        let key = SealedSecretStore::key_at(&dir.path().join("secrets.key")).expect("key");
        let file = dir.path().join("secrets.json");
        let ids: Vec<SecretId> = (0..8).map(|_| SecretId::new()).collect();
        std::thread::scope(|scope| {
            for id in &ids {
                let store = SealedSecretStore::open(&file, key).expect("store");
                scope.spawn(move || store.set(*id, &secrets()).expect("set"));
            }
        });
        let reader = SealedSecretStore::open(&file, key).expect("reader");
        assert!(ids.iter().all(|id| reader.get(*id).is_some()));
    }

    fn secrets() -> Secrets {
        Secrets::from([("api_key".to_owned(), "lin_api_very_secret".to_owned())])
    }

    #[test]
    fn secrets_survive_reopening_and_never_sit_in_the_clear() {
        let dir = tempfile::tempdir().expect("dir");
        let (path, key) =
            (dir.path().join("secrets.json"), SealedSecretStore::key_at(&dir.path().join("secrets.key")).expect("key"));
        let connection = SecretId::new();
        SealedSecretStore::open(&path, key).expect("open").set(connection, &secrets()).expect("set");
        let reopened = SealedSecretStore::open(&path, key).expect("reopen");
        assert_eq!(reopened.get(connection), Some(secrets()));
        assert!(!std::fs::read_to_string(&path).expect("read").contains("very_secret"));
    }

    #[test]
    fn another_key_opens_nothing_and_the_key_is_kept() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("secrets.json");
        let key_path = dir.path().join("secrets.key");
        let key = SealedSecretStore::key_at(&key_path).expect("key");
        assert_eq!(SealedSecretStore::key_at(&key_path).expect("again"), key, "made once, then read");
        let connection = SecretId::new();
        SealedSecretStore::open(&path, key).expect("open").set(connection, &secrets()).expect("set");
        let stranger = SealedSecretStore::open(&path, [9; 32]).expect("open");
        assert_eq!(stranger.get(connection), None);
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
        let (a, b) = (SecretId::new(), SecretId::new());
        store.set(a, &secrets()).expect("set");
        let moved = store.vault.update(|entries: &mut Entries| {
            let sealed = entries.get(&name(a)).cloned();
            entries.extend(sealed.clone().map(|sealed| (name(b), sealed)));
            sealed.is_some()
        });
        assert!(moved.expect("moved"), "the sealed entry now also sits under the other connection");
        assert_eq!(store.get(a), Some(secrets()));
        assert_eq!(store.get(b), None);
    }

    #[test]
    fn a_secrets_file_that_does_not_parse_is_kept_aside_and_a_fresh_one_opens() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("secrets.json");
        std::fs::write(&path, "{ not json").expect("write");

        let store = SealedSecretStore::open(&path, [3; 32]).expect("a fresh store opens");

        let connection = SecretId::new();
        store.set(connection, &secrets()).expect("set");
        assert_eq!(store.get(connection), Some(secrets()));
        let aside: Vec<String> = std::fs::read_dir(dir.path())
            .expect("list")
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with("secrets.json.corrupt-"))
            .collect();
        assert_eq!(aside.len(), 1, "{aside:?}");
        assert_eq!(std::fs::read_to_string(dir.path().join(&aside[0])).expect("aside"), "{ not json");
    }

    #[test]
    fn entries_this_key_cannot_open_are_said_and_never_overwritten() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("secrets.json");
        let connection = SecretId::new();
        SealedSecretStore::open(&path, [3; 32]).expect("open").set(connection, &secrets()).expect("set");
        let before = std::fs::read(&path).expect("read");

        let stranger = SealedSecretStore::open(&path, [9; 32]).expect("open");

        let said = stranger.unkept().expect("the unreadable entry is said");
        assert!(said.contains("cannot be decrypted") && said.contains(&path.display().to_string()), "{said}");
        assert_eq!(stranger.get(connection), None);
        let updated = stranger.update(connection, &mut |secrets| {
            secrets.insert("other".into(), "x".into());
        });
        assert!(updated.expect_err("refused").contains("cannot be decrypted"));
        let replaced = stranger.set(connection, &secrets());
        assert!(replaced.expect_err("refused").contains("cannot be decrypted"));
        assert_eq!(std::fs::read(&path).expect("read"), before, "the sealed entries are as they were");
        assert_eq!(SealedSecretStore::open(&path, [3; 32]).expect("open").get(connection), Some(secrets()));
    }

    #[test]
    fn an_entry_that_stops_opening_after_the_store_opened_is_said_on_the_next_read() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("secrets.json");
        let store = SealedSecretStore::open(&path, [3; 32]).expect("open");
        assert_eq!(store.unkept(), None);
        let connection = SecretId::new();
        SealedSecretStore::open(&path, [9; 32]).expect("open").set(connection, &secrets()).expect("set");

        assert_eq!(store.get(connection), None);

        assert!(store.unkept().expect("said").contains("cannot be decrypted"));
    }

    #[test]
    fn forgetting_removes_the_entry() {
        let dir = tempfile::tempdir().expect("dir");
        let store = SealedSecretStore::open(&dir.path().join("secrets.json"), [3; 32]).expect("open");
        let connection = SecretId::new();
        store.set(connection, &secrets()).expect("set");
        store.delete(connection);
        assert_eq!(store.get(connection), None);
    }
}
