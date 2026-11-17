mod key;
mod sealed;
mod unkept;

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::{Arc, Mutex};

use platform::sync::Locked;

use domain::SecretId;

pub use self::sealed::SealedSecretStore;
pub use self::unkept::{Unkept, refusal};

pub type Secrets = BTreeMap<String, String>;

pub trait SecretStore: Send + Sync {
    fn get(&self, connection: SecretId) -> Option<Secrets>;
    fn set(&self, connection: SecretId, secrets: &Secrets) -> Result<(), String>;
    fn delete(&self, connection: SecretId);

    fn update(&self, connection: SecretId, change: &mut dyn FnMut(&mut Secrets)) -> Result<(), String> {
        let mut secrets = self.get(connection).unwrap_or_default();
        change(&mut secrets);
        self.set(connection, &secrets)
    }

    fn unkept(&self) -> Option<String> {
        None
    }
}

pub fn system_secrets() -> Arc<dyn SecretStore> {
    static STORE: std::sync::OnceLock<Arc<dyn SecretStore>> = std::sync::OnceLock::new();
    Arc::clone(STORE.get_or_init(|| secrets_in(crate::DataRoot::system().secrets().as_deref())))
}

pub fn secrets_in(dir: Option<&Path>) -> Arc<dyn SecretStore> {
    match sealed_in(dir) {
        Ok(store) => Arc::new(store),
        Err(reason) => {
            tracing::error!(%reason, "the sealed secrets file is unusable; no key can be kept");
            Arc::new(Unkept::because(reason))
        }
    }
}

fn sealed_in(dir: Option<&Path>) -> Result<SealedSecretStore, String> {
    let dir = dir.ok_or("this user has no data folder")?;
    std::fs::create_dir_all(dir).map_err(|error| format!("{} cannot be made: {error}", dir.display()))?;
    let key = SealedSecretStore::key_at(&dir.join("secrets.key"))?;
    SealedSecretStore::open(&dir.join("secrets.json"), key)
}

#[derive(Debug, Default)]
pub struct InMemorySecretStore {
    secrets: Mutex<HashMap<SecretId, Secrets>>,
}

impl SecretStore for InMemorySecretStore {
    fn get(&self, connection: SecretId) -> Option<Secrets> {
        let secrets = self.secrets.locked();
        secrets.get(&connection).cloned()
    }

    fn set(&self, connection: SecretId, secrets: &Secrets) -> Result<(), String> {
        let mut map = self.secrets.locked();
        map.insert(connection, secrets.clone());
        Ok(())
    }

    fn delete(&self, connection: SecretId) {
        let mut map = self.secrets.locked();
        map.remove(&connection);
    }

    fn update(&self, connection: SecretId, change: &mut dyn FnMut(&mut Secrets)) -> Result<(), String> {
        change(self.secrets.locked().entry(connection).or_default());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stored(store: &dyn SecretStore) -> Result<(), String> {
        store.set(SecretId::new(), &Secrets::from([("k".to_owned(), "v".to_owned())]))
    }

    #[test]
    fn a_data_folder_gets_a_sealed_file_that_keeps_what_is_set() {
        let dir = tempfile::tempdir().expect("dir");
        let store = secrets_in(Some(&dir.path().join("data")));
        assert_eq!(store.unkept(), None);
        stored(&*store).expect("kept");
        assert!(dir.path().join("data/secrets.json").is_file());
    }

    #[test]
    fn without_a_data_folder_nothing_is_kept_and_no_temporary_folder_stands_in() {
        let store = secrets_in(None);
        let reason = store.unkept().expect("says why");
        assert!(reason.contains("no data folder"), "{reason}");
        let refused = stored(&*store).expect_err("nothing pretends to be saved");
        assert_eq!(refused, format!("keys cannot be kept on this machine: {reason}"));
    }

    #[test]
    fn a_vault_that_cannot_be_opened_says_why_instead_of_keeping_keys_in_memory() {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::create_dir(dir.path().join("secrets.key")).expect("a folder where the key should be");
        let store = secrets_in(Some(dir.path()));
        let reason = store.unkept().expect("says why");
        assert!(reason.contains("secrets.key"), "{reason}");
        assert!(stored(&*store).is_err());
    }
}
