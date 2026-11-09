//! The system's store under the [`Slots`] seam.

use crate::{Error, Slots, entry};

#[derive(Debug, Clone, Copy)]
pub struct System<'a> {
    pub service: &'a str,
}

#[cfg(target_os = "macos")]
type Backing<'a> = crate::migrate::Migrating<crate::protected::DataProtection<'a>, Keyring<'a>>;
#[cfg(not(target_os = "macos"))]
type Backing<'a> = Keyring<'a>;

impl<'a> System<'a> {
    #[cfg(target_os = "macos")]
    fn backing(&self) -> Backing<'a> {
        crate::migrate::Migrating {
            protected: crate::protected::DataProtection { service: self.service },
            legacy: Keyring { service: self.service },
        }
    }

    #[cfg(not(target_os = "macos"))]
    fn backing(&self) -> Backing<'a> {
        Keyring { service: self.service }
    }
}

impl Slots for System<'_> {
    fn read(&self, account: &str) -> Option<String> {
        self.backing().read(account)
    }

    fn write(&self, account: &str, value: &str) -> Result<(), Error> {
        self.backing().write(account, value)
    }

    fn remove(&self, account: &str) {
        self.backing().remove(account);
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Keyring<'a> {
    pub service: &'a str,
}

impl Slots for Keyring<'_> {
    fn read(&self, account: &str) -> Option<String> {
        entry(self.service, account).ok()?.get_password().ok()
    }

    fn write(&self, account: &str, value: &str) -> Result<(), Error> {
        let item = entry(self.service, account).map_err(|error| Error(error.to_string()))?;
        item.set_password(value).map_err(|error| Error(error.to_string()))
    }

    fn remove(&self, account: &str) {
        if let Ok(item) = entry(self.service, account) {
            let _ = item.delete_credential();
        }
    }
}
