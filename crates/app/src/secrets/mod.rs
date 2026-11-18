mod key;
mod sealed;
mod unkept;

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::{Arc, Mutex};

use platform::sync::Locked;

use domain::SecretId;

use self::key::Keeper;

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

    fn hold_for_refresh(&self, _connection: SecretId) -> Result<Option<domain::provider::RefreshHold>, String> {
        Ok(None)
    }
}

pub fn system_secrets() -> Arc<dyn SecretStore> {
    static STORE: std::sync::OnceLock<Arc<dyn SecretStore>> = std::sync::OnceLock::new();
    Arc::clone(STORE.get_or_init(|| {
        let keychain = platform::keychain::SystemKeychain::new(brand::NAME);
        let choice = std::env::var(key::CHOICE).ok();
        kept_in(crate::DataRoot::system().secrets().as_deref(), system_keeper(choice.as_deref(), &keychain))
    }))
}

fn system_keeper<'a>(choice: Option<&str>, keychain: &'a dyn platform::keychain::Keychain) -> Keeper<'a> {
    if cfg!(test) { Keeper::File } else { Keeper::chosen(choice, keychain) }
}

pub fn secrets_in(dir: Option<&Path>) -> Arc<dyn SecretStore> {
    kept_in(dir, Keeper::File)
}

fn kept_in(dir: Option<&Path>, keeper: Keeper<'_>) -> Arc<dyn SecretStore> {
    match sealed_in(dir, keeper) {
        Ok(store) => Arc::new(store),
        Err(reason) => {
            tracing::error!(%reason, "the sealed secrets file is unusable; no key can be kept");
            Arc::new(Unkept::because(reason))
        }
    }
}

fn sealed_in(dir: Option<&Path>, keeper: Keeper<'_>) -> Result<SealedSecretStore, String> {
    let dir = dir.ok_or("this user has no data folder")?;
    std::fs::create_dir_all(dir).map_err(|error| format!("{} cannot be made: {error}", dir.display()))?;
    let key = key::at(&dir.join("secrets.key"), keeper)?;
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

    #[derive(Debug, Default)]
    struct Keychain {
        item: Mutex<Option<String>>,
        gone: std::sync::atomic::AtomicBool,
    }

    impl platform::keychain::Keychain for Keychain {
        fn get(&self, _: &str) -> Result<Option<String>, platform::keychain::Unreachable> {
            if self.gone.load(std::sync::atomic::Ordering::SeqCst) {
                return Err(platform::keychain::Unreachable("locked out".to_owned()));
            }
            Ok(self.item.locked().clone())
        }

        fn set(&self, _: &str, secret: &str) -> Result<(), platform::keychain::Unreachable> {
            *self.item.locked() = Some(secret.to_owned());
            Ok(())
        }
    }

    #[test]
    fn a_vault_whose_keychain_is_gone_keeps_nothing_and_says_how_to_fall_back_to_a_file() {
        let dir = tempfile::tempdir().expect("dir");
        let keychain = Keychain::default();
        let connection = SecretId::new();
        let secrets = Secrets::from([("k".to_owned(), "v".to_owned())]);
        kept_in(Some(dir.path()), Keeper::Keychain(&keychain)).set(connection, &secrets).expect("kept");
        assert_eq!(kept_in(Some(dir.path()), Keeper::Keychain(&keychain)).get(connection), Some(secrets));
        keychain.gone.store(true, std::sync::atomic::Ordering::SeqCst);

        let store = kept_in(Some(dir.path()), Keeper::Keychain(&keychain));

        let reason = store.unkept().expect("says why");
        assert!(reason.contains("locked out") && reason.contains("_SECRETS=file"), "{reason}");
        assert!(stored(&*store).is_err());
        assert!(
            std::fs::read_to_string(dir.path().join("secrets.json")).expect("vault").contains(&connection.to_string())
        );
    }

    #[test]
    fn unit_tests_that_open_the_system_vault_never_reach_the_real_keychain() {
        let keychain = Keychain::default();
        assert!(matches!(system_keeper(None, &keychain), Keeper::File));
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
