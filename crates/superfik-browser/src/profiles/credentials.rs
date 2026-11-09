//! Proxy passwords stay in the OS credential store, indexed by profile ID.
use superfik_core::browser_profiles::ProfileId;

#[derive(Clone)]
pub struct SecretChange {
    pub id: ProfileId,
    pub password: Option<String>,
}
impl std::fmt::Debug for SecretChange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecretChange").field("id", &self.id).field("password", &"[redacted]").finish()
    }
}

pub trait CredentialStore: Send + Sync {
    fn read(&self, id: ProfileId) -> Result<Option<String>, String>;
    fn write(&self, id: ProfileId, password: Option<&str>) -> Result<(), String>;
}

#[derive(Debug)]
pub struct KeychainCredentials;
#[cfg(not(target_arch = "wasm32"))]
impl CredentialStore for KeychainCredentials {
    fn read(&self, id: ProfileId) -> Result<Option<String>, String> {
        let account = id.to_string();
        Ok(superfik_keychain::Item { service: SERVICE, account: &account }.get())
    }
    fn write(&self, id: ProfileId, password: Option<&str>) -> Result<(), String> {
        let account = id.to_string();
        let item = superfik_keychain::Item { service: SERVICE, account: &account };
        match password {
            Some(password) => item.set(password).map_err(|e| e.to_string()),
            None => {
                item.delete();
                Ok(())
            }
        }
    }
}
#[cfg(not(target_arch = "wasm32"))]
const SERVICE: &str = "Superfik Browser Proxy";
