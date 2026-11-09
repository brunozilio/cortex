//! One item in the system's credential store: the macOS Keychain, the
//! Windows Credential Manager, Linux's Secret Service (a `0600` file where
//! it has no keyring, keyutils without a session bus; [`store`]).
//!
//! Windows caps a credential at 2560 bytes, which an OAuth token pair can
//! pass (ADR 0013). There a value too long for one credential is kept in
//! parts: the item holds a marker naming how many, and `<account>#1`… hold
//! the parts. A value that fits is kept whole. The splitting is pure
//! ([`Parted`] over any [`Slots`]); [`Item`] puts it on the system store.
//!
//! On macOS the items live in the data protection keychain under
//! [`ACCESS_GROUP`], so no build of the team is asked to allow them
//! ([`migrate`]).

use std::fmt;

#[cfg(any(target_os = "linux", test))]
pub mod file_store;
pub mod migrate;
#[cfg(target_os = "macos")]
mod protected;
pub mod store;
mod system;

pub use system::{Keyring, System};

pub const ACCESS_GROUP: &str = "F3BSQ3FZTF.superfik.credentials";

pub use keyring;
pub use store::entry;

const MARKER: &str = "superfik-parts:";

const WINDOWS_PART: usize = 600;

pub fn part_limit() -> Option<usize> {
    cfg!(windows).then_some(WINDOWS_PART)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

pub trait Slots {
    fn read(&self, account: &str) -> Option<String>;
    fn write(&self, account: &str, value: &str) -> Result<(), Error>;
    fn remove(&self, account: &str);
}

pub struct Parted<'a, S: Slots> {
    pub slots: &'a S,
    pub account: &'a str,
    pub limit: Option<usize>,
}

impl<S: Slots> fmt::Debug for Parted<'_, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Parted").field("account", &self.account).field("limit", &self.limit).finish_non_exhaustive()
    }
}

impl<S: Slots> Parted<'_, S> {
    pub fn get(&self) -> Option<String> {
        let head = self.slots.read(self.account)?;
        let Some(count) = head.strip_prefix(MARKER) else { return Some(head) };
        let count: usize = count.parse().ok()?;
        (1..=count).map(|n| self.slots.read(&part(self.account, n))).collect()
    }

    pub fn set(&self, value: &str) -> Result<(), Error> {
        let old = self.count();
        let chunks = match self.limit {
            Some(limit) if value.chars().count() > limit || value.starts_with(MARKER) => split(value, limit),
            _ => Vec::new(),
        };
        if chunks.is_empty() {
            self.slots.write(self.account, value)?;
        } else {
            for (n, chunk) in chunks.iter().enumerate() {
                self.slots.write(&part(self.account, n + 1), chunk)?;
            }
            self.slots.write(self.account, &format!("{MARKER}{}", chunks.len()))?;
        }
        for n in chunks.len() + 1..=old {
            self.slots.remove(&part(self.account, n));
        }
        Ok(())
    }

    pub fn delete(&self) {
        for n in 1..=self.count() {
            self.slots.remove(&part(self.account, n));
        }
        self.slots.remove(self.account);
    }

    fn count(&self) -> usize {
        self.slots.read(self.account).and_then(|head| head.strip_prefix(MARKER)?.parse().ok()).unwrap_or(0)
    }
}

fn part(account: &str, n: usize) -> String {
    format!("{account}#{n}")
}

fn split(value: &str, limit: usize) -> Vec<String> {
    let chars: Vec<char> = value.chars().collect();
    chars.chunks(limit.max(1)).map(|chunk| chunk.iter().collect()).collect()
}

#[derive(Debug, Clone, Copy)]
pub struct Item<'a> {
    pub service: &'a str,
    pub account: &'a str,
}

impl Item<'_> {
    fn parted<'s>(&'s self, slots: &'s System<'s>) -> Parted<'s, System<'s>> {
        Parted { slots, account: self.account, limit: part_limit() }
    }

    pub fn get(&self) -> Option<String> {
        self.parted(&System { service: self.service }).get()
    }

    pub fn set(&self, value: &str) -> Result<(), Error> {
        self.parted(&System { service: self.service }).set(value)
    }

    pub fn delete(&self) {
        self.parted(&System { service: self.service }).delete();
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::BTreeMap;

    use super::*;

    #[derive(Default)]
    struct Capped {
        cap: usize,
        items: RefCell<BTreeMap<String, String>>,
    }

    impl Slots for Capped {
        fn read(&self, account: &str) -> Option<String> {
            self.items.borrow().get(account).cloned()
        }

        fn write(&self, account: &str, value: &str) -> Result<(), Error> {
            if value.chars().count() > self.cap {
                return Err(Error("too long".into()));
            }
            self.items.borrow_mut().insert(account.into(), value.into());
            Ok(())
        }

        fn remove(&self, account: &str) {
            self.items.borrow_mut().remove(account);
        }
    }

    fn capped() -> Capped {
        Capped { cap: 20, ..Capped::default() }
    }

    #[test]
    fn a_long_value_survives_a_store_that_caps_each_credential() {
        let store = capped();
        let item = Parted { slots: &store, account: "local/c1", limit: Some(10) };
        let token = "ação-".repeat(9);
        item.set(&token).expect("kept in parts");
        assert_eq!(item.get(), Some(token));
        assert_eq!(store.read("local/c1").as_deref(), Some("superfik-parts:5"));
    }

    #[test]
    fn a_short_value_is_kept_as_it_always_was() {
        let store = capped();
        let item = Parted { slots: &store, account: "a", limit: Some(10) };
        item.set("short").expect("kept");
        assert_eq!(store.read("a").as_deref(), Some("short"));
        assert_eq!(item.get().as_deref(), Some("short"));
    }

    #[test]
    fn shrinking_or_deleting_leaves_no_parts_behind() {
        let store = capped();
        let item = Parted { slots: &store, account: "a", limit: Some(10) };
        item.set(&"x".repeat(35)).expect("four parts");
        item.set(&"y".repeat(12)).expect("two parts");
        assert_eq!(item.get(), Some("y".repeat(12)));
        assert!(store.read("a#3").is_none() && store.read("a#4").is_none());
        item.delete();
        assert!(store.items.borrow().is_empty());
    }

    #[test]
    fn a_value_that_looks_like_the_marker_is_still_itself() {
        let store = capped();
        let item = Parted { slots: &store, account: "a", limit: Some(100) };
        item.set("superfik-parts:2").expect("kept");
        assert_eq!(item.get().as_deref(), Some("superfik-parts:2"));
    }

    #[test]
    fn without_a_limit_nothing_is_split() {
        let store = Capped { cap: usize::MAX, ..Capped::default() };
        let item = Parted { slots: &store, account: "a", limit: None };
        item.set(&"z".repeat(5000)).expect("kept");
        assert_eq!(store.items.borrow().len(), 1);
    }
}
