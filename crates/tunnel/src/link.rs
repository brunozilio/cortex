use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use sha2::{Digest, Sha256};

use crate::keys::{KEY_BYTES, Key, key};

const PREFIX: &str = "pair=";
const PAIRING_ID_BYTES: usize = 8;
const PAIRING_ID_LABEL: &[u8] = b"pairing-id";

pub type PairingId = [u8; PAIRING_ID_BYTES];

#[derive(Clone, PartialEq, Eq)]
pub struct PairingLink {
    pub machine: Key,
    pub code: Key,
}

impl std::fmt::Debug for PairingLink {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("PairingLink").field("machine", &crate::keys::hex(&self.machine)).finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LinkError {
    #[error("a pairing link starts with `{PREFIX}`")]
    Prefix,
    #[error("a pairing link holds a machine key and a code, separated by a dot")]
    Shape,
    #[error("a pairing link is written in base64url: {0}")]
    Encoding(String),
    #[error("a pairing link's machine key and code are {KEY_BYTES} bytes each")]
    Length,
}

impl PairingLink {
    pub fn fragment(&self) -> String {
        format!("{PREFIX}{}.{}", URL_SAFE_NO_PAD.encode(self.machine), URL_SAFE_NO_PAD.encode(self.code))
    }

    pub fn url(&self, page: &str) -> String {
        format!("{}/#{}", page.trim_end_matches('/'), self.fragment())
    }

    pub fn parse(fragment: &str) -> Result<Self, LinkError> {
        let body = fragment.trim_start_matches('#').strip_prefix(PREFIX).ok_or(LinkError::Prefix)?;
        let (machine, code) = body.split_once('.').ok_or(LinkError::Shape)?;
        Ok(Self { machine: decoded(machine)?, code: decoded(code)? })
    }

    pub fn psk(&self) -> Key {
        psk(&self.code)
    }

    pub fn pairing_id(&self) -> PairingId {
        pairing_id(&self.code)
    }
}

pub fn psk(code: &Key) -> Key {
    Sha256::digest(code).into()
}

pub fn pairing_id(code: &Key) -> PairingId {
    let digest = Sha256::new().chain_update(PAIRING_ID_LABEL).chain_update(code).finalize();
    let mut id = [0; PAIRING_ID_BYTES];
    id.copy_from_slice(&digest[..PAIRING_ID_BYTES]);
    id
}

fn decoded(text: &str) -> Result<Key, LinkError> {
    let bytes = URL_SAFE_NO_PAD.decode(text).map_err(|error| LinkError::Encoding(error.to_string()))?;
    key(&bytes).map_err(|_| LinkError::Length)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "a failed step fails the test")]
mod tests {
    use super::*;

    fn link() -> PairingLink {
        PairingLink { machine: [7; KEY_BYTES], code: [9; KEY_BYTES] }
    }

    #[test]
    fn a_link_reads_back_from_its_fragment_with_or_without_the_hash() {
        let link = link();
        assert_eq!(PairingLink::parse(&link.fragment()), Ok(link.clone()));
        assert_eq!(PairingLink::parse(&format!("#{}", link.fragment())), Ok(link));
    }

    #[test]
    fn the_secret_rides_in_the_fragment_which_browsers_keep_from_the_server() {
        let url = link().url("https://relay.example/");
        let (before, after) = url.split_once('#').unwrap();
        assert_eq!(before, "https://relay.example/");
        assert_eq!(PairingLink::parse(after), Ok(link()));
    }

    #[test]
    fn a_malformed_link_says_what_is_wrong() {
        assert_eq!(PairingLink::parse("other=1"), Err(LinkError::Prefix));
        assert_eq!(PairingLink::parse("pair=abc"), Err(LinkError::Shape));
        assert!(matches!(PairingLink::parse("pair=!!.!!"), Err(LinkError::Encoding(_))));
        assert_eq!(PairingLink::parse("pair=AAAA.AAAA"), Err(LinkError::Length));
    }

    #[test]
    fn the_pre_shared_key_and_the_pairing_id_both_follow_the_code_and_differ_from_each_other() {
        let other = PairingLink { code: [8; KEY_BYTES], ..link() };
        assert_ne!(link().psk(), other.psk());
        assert_ne!(link().pairing_id(), other.pairing_id());
        assert_ne!(&link().psk()[..PAIRING_ID_BYTES], &link().pairing_id());
    }

    #[test]
    fn debug_output_never_shows_the_code() {
        assert!(!format!("{:?}", link()).contains("0909"));
    }
}
