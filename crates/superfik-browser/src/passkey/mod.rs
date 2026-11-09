//! Answering a page's passkey calls from the vault, as the Bitwarden
//! extension does. A script in the page's own world ([`shim_script`])
//! wraps `navigator.credentials.get` and `create`: a `publicKey` request
//! goes to the app through a binding ([`BINDING`]) and waits; the app
//! answers with a signature, a new passkey, a refusal, or "use another
//! device", which runs the original call so Chromium's own authenticators
//! (Touch ID, a phone, a security key) still work. A conditional request
//! (the passkey offered in a sign-in field) waits until the user picks;
//! the page's `AbortSignal` withdraws any. The origin is never taken from
//! the script: the shell reads it from the execution context. Pure.

use serde::Deserialize;
use superfik_core::passkey::{Assertion, Attestation};

pub const BINDING: &str = "__superfikPasskey";
const ES256: i64 = -7;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PasskeyAsk {
    pub request: u64,
    pub origin: String,
    pub rp_id: String,
    pub challenge: String,
    pub allowed: Vec<String>,
    pub conditional: bool,
    pub registration: Option<Registration>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Registration {
    pub rp_name: String,
    pub user_id: String,
    pub user_name: String,
    pub user_display_name: String,
    pub es256: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PasskeyReply {
    Signed(Assertion),
    Created(Attestation),
    Elsewhere,
    Refused,
}

#[derive(Deserialize)]
struct Payload {
    request: u64,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    conditional: bool,
    #[serde(rename = "rpId")]
    rp_id: Option<String>,
    #[serde(rename = "rpName", default)]
    rp_name: String,
    challenge: String,
    #[serde(default)]
    allowed: Vec<String>,
    #[serde(default)]
    excluded: Vec<String>,
    user: Option<User>,
    #[serde(default)]
    algorithms: Vec<i64>,
}

#[derive(Deserialize)]
struct User {
    id: String,
    #[serde(default)]
    name: String,
    #[serde(rename = "displayName", default)]
    display_name: String,
}

impl PasskeyAsk {
    pub fn read(payload: &str, origin: &str) -> Option<Self> {
        let payload: Payload = serde_json::from_str(payload).ok()?;
        let host = url::Url::parse(origin).ok()?.host_str()?.to_owned();
        let registration = match payload.kind.as_str() {
            "create" => {
                let user = payload.user?;
                Some(Registration {
                    rp_name: payload.rp_name,
                    user_id: user.id,
                    user_name: user.name,
                    user_display_name: user.display_name,
                    es256: payload.algorithms.is_empty() || payload.algorithms.contains(&ES256),
                })
            }
            _ => None,
        };
        let allowed = if registration.is_some() { payload.excluded } else { payload.allowed };
        Some(Self {
            request: payload.request,
            origin: origin.to_owned(),
            rp_id: payload.rp_id.filter(|r| !r.is_empty()).unwrap_or(host),
            challenge: payload.challenge,
            allowed,
            conditional: payload.conditional && registration.is_none(),
            registration,
        })
    }
}

pub fn shim_script(answer: &str) -> String {
    include_str!("shim.js").replace("__ANSWER__", answer).replace("__BINDING__", BINDING)
}

pub fn answer_script(answer: &str, request: u64, reply: &PasskeyReply) -> String {
    let reply = match reply {
        PasskeyReply::Signed(assertion) => serde_json::json!({ "signed": assertion }),
        PasskeyReply::Created(attestation) => serde_json::json!({ "created": attestation }),
        PasskeyReply::Elsewhere => serde_json::json!({ "elsewhere": true }),
        PasskeyReply::Refused => serde_json::json!({}),
    };
    format!("window.{answer} && window.{answer}({request}, {reply})")
}

#[cfg(test)]
mod tests;
