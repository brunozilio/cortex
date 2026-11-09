//! Which store keeps credentials on Linux. The desktop's Secret Service
//! (GNOME Keyring, KWallet) keeps them across restarts when it has a
//! keyring to keep them in. A session signed in automatically has none
//! (GNOME makes its "Login" keyring at a password sign-in), and creating
//! one asks for a password, so there, and on a desktop with no Secret
//! Service, they go to a file only the user can read
//! ([`crate::file_store`]), as Chromium's `password-store=basic` does;
//! once that file exists it stays the store, so a keyring made later never
//! hides what it keeps. With no session bus at all (a server in a
//! container, which seals its connection secrets in a file instead) the
//! kernel's keyutils serves, forgetting them on restart. macOS and Windows
//! have one store each and choose nothing.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Store {
    SecretService,
    File,
    Keyutils,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Service {
    Absent,
    NoKeyring,
    Ready,
}

pub fn choose(file_exists: bool, session_bus: bool, service: impl FnOnce() -> Service) -> Store {
    if file_exists {
        Store::File
    } else if !session_bus {
        Store::Keyutils
    } else if service() == Service::Ready {
        Store::SecretService
    } else {
        Store::File
    }
}

pub fn entry(service: &str, account: &str) -> keyring::Result<keyring::Entry> {
    #[cfg(target_os = "linux")]
    {
        static CHOSEN: std::sync::OnceLock<Store> = std::sync::OnceLock::new();
        CHOSEN.get_or_init(use_system_store);
    }
    keyring::Entry::new(service, account)
}

#[cfg(target_os = "linux")]
fn use_system_store() -> Store {
    let session_bus = std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some_and(|address| !address.is_empty());
    let file = crate::file_store::default_path();
    let file_exists = file.as_ref().is_some_and(|path| path.exists());
    let mut store = choose(file_exists, session_bus, secret_service);
    keyring::set_default_credential_builder(match (store, file) {
        (Store::SecretService, _) => keyring::secret_service::default_credential_builder(),
        (Store::File, Some(path)) => Box::new(crate::file_store::FileStore::new(path)),
        _ => {
            store = Store::Keyutils;
            keyring::keyutils::default_credential_builder()
        }
    });
    tracing::info!(?store, "credentials store");
    store
}

#[cfg(target_os = "linux")]
fn secret_service() -> Service {
    use dbus_secret_service::{EncryptionType, SecretService};
    match SecretService::connect(EncryptionType::Plain) {
        Err(_) => Service::Absent,
        Ok(service) if service.get_default_collection().is_ok() => Service::Ready,
        Ok(_) => Service::NoKeyring,
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    #[test]
    fn a_desktop_with_a_keyring_keeps_credentials_in_the_secret_service() {
        assert_eq!(choose(false, true, || Service::Ready), Store::SecretService);
    }

    #[test]
    fn a_desktop_without_a_keyring_keeps_them_in_the_file_and_asks_nothing() {
        assert_eq!(choose(false, true, || Service::NoKeyring), Store::File);
        assert_eq!(choose(false, true, || Service::Absent), Store::File);
    }

    #[test]
    fn once_the_file_exists_it_stays_the_store() {
        let asked = Cell::new(false);
        let store = choose(true, true, || {
            asked.set(true);
            Service::Ready
        });
        assert_eq!(store, Store::File);
        assert!(!asked.get());
    }

    #[test]
    fn a_headless_process_uses_keyutils_and_never_asks_the_bus() {
        let asked = Cell::new(false);
        let store = choose(false, false, || {
            asked.set(true);
            Service::Ready
        });
        assert_eq!(store, Store::Keyutils);
        assert!(!asked.get());
    }
}
