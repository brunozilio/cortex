//! Connection secrets (tokens, API keys). Only the backend reads them; the
//! client never receives one.

use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;

use superfik_core::{ConnectionId, Principal};

use crate::memory::owner_key;

pub type Secrets = BTreeMap<String, String>;

pub trait SecretStore: Send + Sync {
    fn get(&self, owner: &Principal, connection: ConnectionId) -> Option<Secrets>;
    fn set(&self, owner: &Principal, connection: ConnectionId, secrets: &Secrets) -> Result<(), String>;
    fn delete(&self, owner: &Principal, connection: ConnectionId);
}

#[derive(Debug, Default, Clone, Copy)]
pub struct KeychainSecretStore;

const SERVICE: &str = "Superfik";

fn account(owner: &Principal, connection: ConnectionId) -> String {
    format!("{}/{connection}", owner_key(owner))
}

impl SecretStore for KeychainSecretStore {
    fn get(&self, owner: &Principal, connection: ConnectionId) -> Option<Secrets> {
        let account = account(owner, connection);
        let json = superfik_keychain::Item { service: SERVICE, account: &account }.get()?;
        serde_json::from_str(&json).ok()
    }

    fn set(&self, owner: &Principal, connection: ConnectionId, secrets: &Secrets) -> Result<(), String> {
        let json = serde_json::to_string(secrets).map_err(|error| error.to_string())?;
        let account = account(owner, connection);
        superfik_keychain::Item { service: SERVICE, account: &account }.set(&json).map_err(|error| error.to_string())
    }

    fn delete(&self, owner: &Principal, connection: ConnectionId) {
        let account = account(owner, connection);
        superfik_keychain::Item { service: SERVICE, account: &account }.delete();
    }
}

#[derive(Debug, Default)]
pub struct InMemorySecretStore {
    secrets: Mutex<HashMap<(String, ConnectionId), Secrets>>,
}

impl SecretStore for InMemorySecretStore {
    fn get(&self, owner: &Principal, connection: ConnectionId) -> Option<Secrets> {
        let secrets = self.secrets.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        secrets.get(&(owner_key(owner), connection)).cloned()
    }

    fn set(&self, owner: &Principal, connection: ConnectionId, secrets: &Secrets) -> Result<(), String> {
        let mut map = self.secrets.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        map.insert((owner_key(owner), connection), secrets.clone());
        Ok(())
    }

    fn delete(&self, owner: &Principal, connection: ConnectionId) {
        let mut map = self.secrets.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        map.remove(&(owner_key(owner), connection));
    }
}
