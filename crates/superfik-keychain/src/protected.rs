//! The data protection keychain on macOS, its items in [`ACCESS_GROUP`].

use security_framework::passwords::{
    PasswordOptions, delete_generic_password_options, generic_password, set_generic_password_options,
};

use crate::migrate::{Lookup, Protected, Refused};
use crate::{ACCESS_GROUP, Error};

const MISSING_ENTITLEMENT: i32 = -34018;
const ITEM_NOT_FOUND: i32 = -25300;

#[derive(Debug, Clone, Copy)]
pub struct DataProtection<'a> {
    pub service: &'a str,
}

impl DataProtection<'_> {
    fn options(&self, account: &str) -> PasswordOptions {
        let mut options = PasswordOptions::new_generic_password(self.service, account);
        options.use_protected_keychain();
        options.set_access_group(ACCESS_GROUP);
        options
    }
}

impl Protected for DataProtection<'_> {
    fn lookup(&self, account: &str) -> Lookup {
        match generic_password(self.options(account)) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(value) => Lookup::Found(value),
                Err(_) => Lookup::Missing,
            },
            Err(error) if error.code() == ITEM_NOT_FOUND => Lookup::Missing,
            Err(error) if error.code() == MISSING_ENTITLEMENT => Lookup::Unavailable,
            Err(error) => {
                tracing::warn!(%error, "the protected keychain did not answer; using the file keychain");
                Lookup::Unavailable
            }
        }
    }

    fn keep(&self, account: &str, value: &str) -> Result<(), Refused> {
        set_generic_password_options(value.as_bytes(), self.options(account)).map_err(|error| {
            if error.code() == MISSING_ENTITLEMENT {
                Refused::Unavailable
            } else {
                Refused::Failed(Error(error.to_string()))
            }
        })
    }

    fn forget(&self, account: &str) {
        let _ = delete_generic_password_options(self.options(account));
    }
}
