//! Passkeys from a vault, as the Bitwarden extension answers a page's
//! `navigator.credentials` call: which saved passkeys a request may use,
//! whether its relying party belongs to the page, and the bytes of the
//! ceremony (client data, authenticator data, attestation). Signing
//! happens where the key is kept (the backend). Pure.

mod ceremony;
mod rp;

pub use ceremony::{
    Ceremony, PublicKey, attestation_object, authenticator_data, client_data_json, credential_bytes, credential_id,
    decode,
};
pub use rp::rp_id_allowed;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredPasskey {
    pub item: String,
    pub account: String,
    pub credential: String,
    pub rp_id: String,
    pub user_name: Option<String>,
    pub discoverable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PasskeyUse {
    pub origin: String,
    pub rp_id: String,
    pub challenge: String,
    pub account: String,
    pub credential: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Assertion {
    pub credential_id: String,
    pub client_data: String,
    pub authenticator_data: String,
    pub signature: String,
    pub user_handle: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewPasskey {
    pub origin: String,
    pub rp_id: String,
    pub rp_name: String,
    pub challenge: String,
    pub user_id: String,
    pub user_name: String,
    pub user_display_name: String,
    pub excluded: Vec<String>,
    pub account: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attestation {
    pub credential_id: String,
    pub client_data: String,
    pub attestation_object: String,
    pub authenticator_data: String,
    pub public_key: String,
}

pub fn offered(stored: &[StoredPasskey], rp_id: &str, allowed: &[String]) -> Vec<StoredPasskey> {
    stored
        .iter()
        .filter(|p| p.rp_id.eq_ignore_ascii_case(rp_id))
        .filter(|p| match allowed.is_empty() {
            true => p.discoverable,
            false => credential_id(&p.credential).is_some_and(|id| allowed.contains(&id)),
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests;
