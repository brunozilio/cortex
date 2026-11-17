use sha2::{Digest, Sha256};
use snow::params::DHChoice;
use snow::resolvers::{CryptoResolver, DefaultResolver};

pub const KEY_BYTES: usize = 32;
const MACHINE_ID_BYTES: usize = 16;

pub type Key = [u8; KEY_BYTES];

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum KeyError {
    #[error("a key is {KEY_BYTES} bytes, this one is {0}")]
    Length(usize),
    #[error("the system gave no X25519: {0}")]
    Unavailable(String),
}

#[derive(Clone, PartialEq, Eq)]
pub struct StaticKey {
    private: Key,
    public: Key,
}

impl std::fmt::Debug for StaticKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("StaticKey").field("public", &hex(&self.public)).finish_non_exhaustive()
    }
}

impl StaticKey {
    pub fn generate() -> Result<Self, KeyError> {
        let pair = snow::Builder::new(crate::handshake::IK.parse().map_err(unavailable)?)
            .generate_keypair()
            .map_err(unavailable)?;
        Self::from_private(&pair.private)
    }

    pub fn from_private(private: &[u8]) -> Result<Self, KeyError> {
        let private = key(private)?;
        let mut dh = DefaultResolver
            .resolve_dh(&DHChoice::Curve25519)
            .ok_or_else(|| KeyError::Unavailable("no Curve25519".into()))?;
        dh.set(&private);
        Ok(Self { private, public: key(dh.pubkey())? })
    }

    pub fn private(&self) -> &Key {
        &self.private
    }

    pub fn public(&self) -> &Key {
        &self.public
    }

    pub fn machine_id(&self) -> String {
        machine_id(&self.public)
    }
}

pub fn key(bytes: &[u8]) -> Result<Key, KeyError> {
    bytes.try_into().map_err(|_| KeyError::Length(bytes.len()))
}

pub fn machine_id(public: &Key) -> String {
    hex(&Sha256::digest(public)[..MACHINE_ID_BYTES])
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn unavailable(error: impl std::fmt::Display) -> KeyError {
    KeyError::Unavailable(error.to_string())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "a failed step fails the test")]
mod tests {
    use super::*;

    #[test]
    fn a_key_read_back_from_its_private_half_has_the_same_public_half() {
        let made = StaticKey::generate().unwrap();
        assert_eq!(StaticKey::from_private(made.private()).unwrap(), made);
    }

    #[test]
    fn two_generated_keys_differ() {
        assert_ne!(StaticKey::generate().unwrap().public(), StaticKey::generate().unwrap().public());
    }

    #[test]
    fn a_private_key_of_the_wrong_length_is_refused() {
        assert_eq!(StaticKey::from_private(&[1; 31]), Err(KeyError::Length(31)));
    }

    #[test]
    fn the_machine_id_is_32_hex_digits_that_follow_the_public_key() {
        let made = StaticKey::generate().unwrap();
        let id = made.machine_id();
        assert_eq!(id.len(), 32);
        assert!(id.chars().all(|digit| digit.is_ascii_hexdigit()));
        assert_eq!(id, machine_id(made.public()));
        assert_ne!(id, StaticKey::generate().unwrap().machine_id());
    }

    #[test]
    fn debug_output_never_shows_the_private_half() {
        let made = StaticKey::generate().unwrap();
        assert!(!format!("{made:?}").contains(&hex(made.private())));
    }
}
