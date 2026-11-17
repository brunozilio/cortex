use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use platform::store::Document;
use serde::{Deserialize, Serialize};
use tunnel::link::{PairingId, pairing_id};
use tunnel::{Admission, Key, PairingLink, StaticKey};

pub const PAIRING_LASTS: Duration = Duration::from_secs(10 * 60);
pub const MOST_PENDING: usize = 4;
const FILE: &str = "remote.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Device {
    pub name: String,
    #[serde(with = "encoded")]
    pub key: Key,
    pub paired_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Pending {
    #[serde(with = "encoded")]
    code: Key,
    expires_at: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ledger {
    #[serde(default, skip_serializing_if = "Option::is_none", with = "encoded_option")]
    machine: Option<Key>,
    #[serde(default)]
    devices: Vec<Device>,
    #[serde(default)]
    pending: Vec<Pending>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    relay: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BookError {
    #[error(transparent)]
    Relay(#[from] super::link::RelayAddressError),
    #[error("the remote control's file could not be kept: {0}")]
    Store(String),
    #[error("the machine's key could not be made: {0}")]
    Key(String),
    #[error("no paired device is named `{0}`")]
    NoSuchDevice(String),
}

impl Ledger {
    fn machine_key(&mut self) -> Result<StaticKey, BookError> {
        if let Some(private) = self.machine {
            return StaticKey::from_private(&private).map_err(|error| BookError::Key(error.to_string()));
        }
        let made = StaticKey::generate().map_err(|error| BookError::Key(error.to_string()))?;
        self.machine = Some(*made.private());
        Ok(made)
    }

    fn forget_expired(&mut self, now: u64) {
        self.pending.retain(|pending| pending.expires_at > now);
    }

    fn pend(&mut self, code: Key, now: u64) {
        self.forget_expired(now);
        if self.pending.len() >= MOST_PENDING {
            self.pending.remove(0);
        }
        self.pending.push(Pending { code, expires_at: now + PAIRING_LASTS.as_secs() });
    }

    fn pending_code(&self, id: &PairingId, now: u64) -> Option<Key> {
        self.pending
            .iter()
            .find(|pending| pending.expires_at > now && &pairing_id(&pending.code) == id)
            .map(|pending| pending.code)
    }

    fn join(&mut self, id: &PairingId, name: &str, key: Key, now: u64) -> Option<Device> {
        let code = self.pending_code(id, now)?;
        self.pending.retain(|pending| pending.code != code);
        self.devices.retain(|device| device.key != key);
        let device = Device { name: self.free_name(name), key, paired_at: now };
        self.devices.push(device.clone());
        Some(device)
    }

    fn free_name(&self, wanted: &str) -> String {
        let taken = |name: &str| self.devices.iter().any(|device| device.name == name);
        (1..)
            .map(|at| if at == 1 { wanted.to_owned() } else { format!("{wanted} ({at})") })
            .find(|name| !taken(name))
            .unwrap_or_default()
    }

    fn revoke(&mut self, name: &str) -> Option<Device> {
        let at = self.devices.iter().position(|device| device.name == name)?;
        Some(self.devices.remove(at))
    }
}

#[derive(Debug, Clone)]
pub struct Book {
    document: Document,
    clock: fn() -> u64,
}

pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs())
}

fn stored(error: platform::store::Error) -> BookError {
    BookError::Store(error.to_string())
}

impl Book {
    pub fn in_folder(folder: &Path) -> Self {
        Self::at(folder.join(FILE), now)
    }

    pub fn at(path: PathBuf, clock: fn() -> u64) -> Self {
        Self { document: Document::at(path), clock }
    }

    pub fn machine(&self) -> Result<StaticKey, BookError> {
        self.document.try_update(Ledger::machine_key).map_err(stored)?
    }

    pub fn invite(&self) -> Result<PairingLink, BookError> {
        let machine = self.machine()?;
        let code = StaticKey::generate().map_err(|error| BookError::Key(error.to_string()))?;
        let now = (self.clock)();
        self.document.update(|ledger: &mut Ledger| ledger.pend(*code.private(), now)).map_err(stored)?;
        Ok(PairingLink { machine: *machine.public(), code: *code.private() })
    }

    pub fn join(&self, id: &PairingId, name: &str, key: Key) -> Result<Option<Device>, BookError> {
        let now = (self.clock)();
        self.document.update(|ledger: &mut Ledger| ledger.join(id, name, key, now)).map_err(stored)
    }

    pub fn devices(&self) -> Vec<Device> {
        self.document.read_or_default::<Ledger>().devices
    }

    pub fn revoke(&self, name: &str) -> Result<Device, BookError> {
        self.document
            .update(|ledger: &mut Ledger| ledger.revoke(name))
            .map_err(stored)?
            .ok_or_else(|| BookError::NoSuchDevice(name.to_owned()))
    }

    pub fn relay(&self) -> Option<String> {
        self.document.read_or_default::<Ledger>().relay
    }

    pub fn set_relay(&self, address: &str) -> Result<String, BookError> {
        let address = super::link::relay_address(address)?;
        let kept = address.clone();
        self.document.update(|ledger: &mut Ledger| ledger.relay = Some(kept)).map_err(stored)?;
        Ok(address)
    }

    pub fn anyone_to_serve(&self) -> bool {
        let ledger = self.document.read_or_default::<Ledger>();
        let now = (self.clock)();
        !ledger.devices.is_empty() || ledger.pending.iter().any(|pending| pending.expires_at > now)
    }
}

impl Admission for Book {
    fn knows(&self, device: &Key) -> bool {
        self.document.read_or_default::<Ledger>().devices.iter().any(|known| &known.key == device)
    }

    fn pending(&self, pairing: &PairingId) -> Option<Key> {
        self.document.read_or_default::<Ledger>().pending_code(pairing, (self.clock)())
    }
}

mod encoded {
    use super::{Engine, Key, STANDARD};
    use serde::{Deserialize, Deserializer, Serializer, de::Error};

    pub fn serialize<S: Serializer>(key: &Key, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&STANDARD.encode(key))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Key, D::Error> {
        let text = String::deserialize(deserializer)?;
        let bytes = STANDARD.decode(text).map_err(D::Error::custom)?;
        bytes.try_into().map_err(|_| D::Error::custom("a key is 32 bytes"))
    }
}

mod encoded_option {
    use super::Key;
    use serde::{Deserialize, Deserializer, Serializer};

    #[allow(clippy::ref_option, reason = "serde hands an optional field by reference")]
    pub fn serialize<S: Serializer>(key: &Option<Key>, serializer: S) -> Result<S::Ok, S::Error> {
        match key {
            Some(key) => super::encoded::serialize(key, serializer),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Key>, D::Error> {
        #[derive(Deserialize)]
        struct Wrapped(#[serde(with = "super::encoded")] Key);
        Option::<Wrapped>::deserialize(deserializer).map(|wrapped| wrapped.map(|Wrapped(key)| key))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "a failed step fails the test")]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static CLOCK: AtomicU64 = AtomicU64::new(1_000);

    fn clock() -> u64 {
        CLOCK.load(Ordering::SeqCst)
    }

    fn book(folder: &Path) -> Book {
        Book::at(folder.join(FILE), clock)
    }

    #[test]
    fn the_machine_keeps_one_key_across_openings() {
        let folder = tempfile::tempdir().unwrap();
        let first = book(folder.path()).machine().unwrap();
        assert_eq!(book(folder.path()).machine().unwrap(), first);
    }

    #[test]
    fn an_invitation_admits_one_device_once_and_names_it() {
        let folder = tempfile::tempdir().unwrap();
        let book = book(folder.path());
        let link = book.invite().unwrap();
        assert_eq!(link.machine, *book.machine().unwrap().public());
        assert_eq!(book.pending(&link.pairing_id()), Some(link.code));
        assert!(book.anyone_to_serve());
        let device = book.join(&link.pairing_id(), "phone", [1; 32]).unwrap().unwrap();
        assert_eq!(device.name, "phone");
        assert!(book.knows(&[1; 32]));
        assert_eq!(book.pending(&link.pairing_id()), None);
        assert_eq!(book.join(&link.pairing_id(), "again", [2; 32]).unwrap(), None);
        assert!(!book.knows(&[2; 32]));
    }

    #[test]
    fn two_devices_with_one_name_are_told_apart_and_a_key_pairs_only_once() {
        let mut ledger = Ledger::default();
        for code in [[1; 32], [2; 32], [3; 32]] {
            ledger.pend(code, 0);
        }
        ledger.join(&pairing_id(&[1; 32]), "phone", [7; 32], 0);
        ledger.join(&pairing_id(&[2; 32]), "phone", [8; 32], 0);
        ledger.join(&pairing_id(&[3; 32]), "tablet", [7; 32], 0);
        let names: Vec<_> = ledger.devices.iter().map(|device| device.name.as_str()).collect();
        assert_eq!(names, ["phone (2)", "tablet"]);
    }

    #[test]
    fn an_invitation_expires_after_ten_minutes() {
        let mut ledger = Ledger::default();
        ledger.pend([1; 32], 0);
        let id = pairing_id(&[1; 32]);
        assert!(ledger.pending_code(&id, PAIRING_LASTS.as_secs() - 1).is_some());
        assert!(ledger.pending_code(&id, PAIRING_LASTS.as_secs()).is_none());
        assert!(ledger.join(&id, "late", [1; 32], PAIRING_LASTS.as_secs()).is_none());
    }

    #[test]
    fn only_the_newest_few_invitations_stay_pending() {
        let mut ledger = Ledger::default();
        for at in 0..=MOST_PENDING as u8 {
            ledger.pend([at; 32], 0);
        }
        assert_eq!(ledger.pending.len(), MOST_PENDING);
        assert!(ledger.pending_code(&pairing_id(&[0; 32]), 0).is_none());
    }

    #[test]
    fn a_revoked_device_is_no_longer_admitted() {
        let folder = tempfile::tempdir().unwrap();
        let book = book(folder.path());
        let link = book.invite().unwrap();
        book.join(&link.pairing_id(), "phone", [1; 32]).unwrap();
        assert_eq!(book.revoke("phone").unwrap().key, [1; 32]);
        assert!(!book.knows(&[1; 32]));
        assert!(!book.anyone_to_serve());
        assert_eq!(book.revoke("phone").unwrap_err(), BookError::NoSuchDevice("phone".into()));
    }

    #[test]
    fn the_relay_is_kept_without_its_trailing_slash_and_a_bad_one_is_refused() {
        let folder = tempfile::tempdir().unwrap();
        let book = book(folder.path());
        assert_eq!(book.relay(), None);
        assert_eq!(book.set_relay(" https://relay.example/ ").unwrap(), "https://relay.example");
        assert_eq!(book.relay().as_deref(), Some("https://relay.example"));
        assert!(book.set_relay("http://relay.example").is_err());
        assert_eq!(book.relay().as_deref(), Some("https://relay.example"));
    }

    #[test]
    fn the_file_is_readable_only_by_its_owner() {
        let folder = tempfile::tempdir().unwrap();
        let book = book(folder.path());
        book.invite().unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(folder.path().join(FILE)).unwrap().permissions().mode();
            assert_eq!(mode & 0o077, 0);
        }
    }
}
