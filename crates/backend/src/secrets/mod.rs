//! Keys and tokens. Only the backend reads them; a client never receives
//! one.

mod sealed;

use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;

use domain::{Principal, SecretId};

use crate::memory::owner_key;

pub use self::sealed::SealedSecretStore;

pub type Secrets = BTreeMap<String, String>;

pub trait SecretStore: Send + Sync {
    fn get(&self, owner: &Principal, connection: SecretId) -> Option<Secrets>;
    fn set(&self, owner: &Principal, connection: SecretId, secrets: &Secrets) -> Result<(), String>;
    fn delete(&self, owner: &Principal, connection: SecretId);
}

/// This computer's store for keys and tokens: a sealed file in the data
/// folder (`secrets.json`, its key in `secrets.key`, both `0600`), shared
/// by everything in the process. When the folder cannot hold it, secrets
/// last only as long as the process.
pub fn system_secrets() -> std::sync::Arc<dyn SecretStore> {
    static STORE: std::sync::OnceLock<std::sync::Arc<dyn SecretStore>> = std::sync::OnceLock::new();
    std::sync::Arc::clone(STORE.get_or_init(|| {
        let dir = crate::local_data_dir().unwrap_or_else(std::env::temp_dir);
        let sealed = std::fs::create_dir_all(&dir)
            .map_err(|error| error.to_string())
            .and_then(|()| SealedSecretStore::key_at(&dir.join("secrets.key")))
            .and_then(|key| SealedSecretStore::open(&dir.join("secrets.json"), key));
        match sealed {
            Ok(store) => std::sync::Arc::new(store),
            Err(error) => {
                tracing::error!(%error, "the sealed secrets file is unusable; keys last only until restart");
                std::sync::Arc::new(InMemorySecretStore::default())
            }
        }
    }))
}

#[derive(Debug, Default)]
pub struct InMemorySecretStore {
    secrets: Mutex<HashMap<(String, SecretId), Secrets>>,
}

impl SecretStore for InMemorySecretStore {
    fn get(&self, owner: &Principal, connection: SecretId) -> Option<Secrets> {
        let secrets = self.secrets.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        secrets.get(&(owner_key(owner), connection)).cloned()
    }

    fn set(&self, owner: &Principal, connection: SecretId, secrets: &Secrets) -> Result<(), String> {
        let mut map = self.secrets.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        map.insert((owner_key(owner), connection), secrets.clone());
        Ok(())
    }

    fn delete(&self, owner: &Principal, connection: SecretId) {
        let mut map = self.secrets.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        map.remove(&(owner_key(owner), connection));
    }
}
