//! macOS keeps two keychains. The file-based one (what `keyring` uses)
//! guards each item with a list of trusted applications, so any build
//! whose signature differs from the writer's (a release beside the
//! development app, an ad hoc build, a bare binary) asks the user to allow
//! it. The data protection keychain grants items by access group instead:
//! every app of the team holding the group reads them without asking.
//!
//! [`Migrating`] keeps values in the protected keychain and moves an item
//! the file keychain still holds there the first time it is read. A process
//! without the group (tests, a bare or ad hoc binary) keeps using the file
//! keychain. Pure: both stores are traits.

use crate::{Error, Slots};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lookup {
    Found(String),
    Missing,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    Unavailable,
    Failed(Error),
}

pub trait Protected {
    fn lookup(&self, account: &str) -> Lookup;
    fn keep(&self, account: &str, value: &str) -> Result<(), Refused>;
    fn forget(&self, account: &str);
}

#[derive(Debug)]
pub struct Migrating<P, L> {
    pub protected: P,
    pub legacy: L,
}

impl<P: Protected, L: Slots> Slots for Migrating<P, L> {
    fn read(&self, account: &str) -> Option<String> {
        match self.protected.lookup(account) {
            Lookup::Found(value) => Some(value),
            Lookup::Unavailable => self.legacy.read(account),
            Lookup::Missing => {
                let value = self.legacy.read(account)?;
                match self.protected.keep(account, &value) {
                    Ok(()) => self.legacy.remove(account),
                    Err(error) => {
                        tracing::warn!(?error, account, "could not move a credential to the protected keychain")
                    }
                }
                Some(value)
            }
        }
    }

    fn write(&self, account: &str, value: &str) -> Result<(), Error> {
        match self.protected.keep(account, value) {
            Ok(()) => Ok(()),
            Err(Refused::Unavailable) => self.legacy.write(account, value),
            Err(Refused::Failed(error)) => Err(error),
        }
    }

    fn remove(&self, account: &str) {
        self.protected.forget(account);
        self.legacy.remove(account);
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::BTreeMap;

    use super::*;

    #[derive(Default)]
    struct Map(RefCell<BTreeMap<String, String>>);

    impl Map {
        fn with(account: &str, value: &str) -> Self {
            let map = Self::default();
            map.0.borrow_mut().insert(account.into(), value.into());
            map
        }
        fn has(&self, account: &str) -> bool {
            self.0.borrow().contains_key(account)
        }
    }

    impl Slots for Map {
        fn read(&self, account: &str) -> Option<String> {
            self.0.borrow().get(account).cloned()
        }
        fn write(&self, account: &str, value: &str) -> Result<(), Error> {
            self.0.borrow_mut().insert(account.into(), value.into());
            Ok(())
        }
        fn remove(&self, account: &str) {
            self.0.borrow_mut().remove(account);
        }
    }

    struct Keychain {
        entitled: bool,
        items: Map,
    }

    impl Keychain {
        fn entitled() -> Self {
            Self { entitled: true, items: Map::default() }
        }
        fn without_group() -> Self {
            Self { entitled: false, items: Map::default() }
        }
    }

    impl Protected for Keychain {
        fn lookup(&self, account: &str) -> Lookup {
            match (self.entitled, self.items.read(account)) {
                (false, _) => Lookup::Unavailable,
                (true, Some(value)) => Lookup::Found(value),
                (true, None) => Lookup::Missing,
            }
        }
        fn keep(&self, account: &str, value: &str) -> Result<(), Refused> {
            if !self.entitled {
                return Err(Refused::Unavailable);
            }
            self.items.write(account, value).map_err(Refused::Failed)
        }
        fn forget(&self, account: &str) {
            self.items.remove(account);
        }
    }

    #[test]
    fn an_entitled_app_keeps_credentials_in_the_protected_keychain() {
        let store = Migrating { protected: Keychain::entitled(), legacy: Map::default() };
        store.write("refresh-token", "token").expect("kept");
        assert_eq!(store.read("refresh-token").as_deref(), Some("token"));
        assert!(store.protected.items.has("refresh-token"));
        assert!(!store.legacy.has("refresh-token"));
    }

    #[test]
    fn an_item_of_the_file_keychain_moves_over_when_first_read() {
        let store = Migrating { protected: Keychain::entitled(), legacy: Map::with("refresh-token", "old") };
        assert_eq!(store.read("refresh-token").as_deref(), Some("old"));
        assert_eq!(store.protected.items.read("refresh-token").as_deref(), Some("old"));
        assert!(!store.legacy.has("refresh-token"));
    }

    #[test]
    fn the_protected_value_wins_over_a_stale_one_in_the_file_keychain() {
        let store = Migrating { protected: Keychain::entitled(), legacy: Map::with("a", "stale") };
        store.protected.items.write("a", "fresh").expect("kept");
        assert_eq!(store.read("a").as_deref(), Some("fresh"));
    }

    #[test]
    fn without_the_group_the_file_keychain_serves_as_before() {
        let store = Migrating { protected: Keychain::without_group(), legacy: Map::with("a", "kept") };
        assert_eq!(store.read("a").as_deref(), Some("kept"));
        store.write("b", "new").expect("kept");
        assert_eq!(store.legacy.read("b").as_deref(), Some("new"));
    }

    #[test]
    fn removing_forgets_the_item_in_both_keychains() {
        let store = Migrating { protected: Keychain::entitled(), legacy: Map::with("a", "old") };
        store.protected.items.write("a", "new").expect("kept");
        store.remove("a");
        assert_eq!(store.read("a"), None);
    }

    #[test]
    fn a_missing_item_is_missing_in_both() {
        let store = Migrating { protected: Keychain::entitled(), legacy: Map::default() };
        assert_eq!(store.read("nothing"), None);
        assert!(!store.protected.items.has("nothing"));
    }
}
