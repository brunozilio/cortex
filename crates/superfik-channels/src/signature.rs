//! Each provider's proof that a webhook call is its own. Every comparison
//! runs in constant time (`Mac::verify_slice`).
//!
//! - Meta (WhatsApp Cloud, Instagram): `X-Hub-Signature-256: sha256=<hex>`,
//!   HMAC-SHA256 of the body with the app secret; the subscription is
//!   verified with `hub.mode=subscribe`, `hub.verify_token` and
//!   `hub.challenge` echoed back.
//! - X (Account Activity): `x-twitter-webhooks-signature: sha256=<base64>`,
//!   HMAC-SHA256 of the body with the consumer secret; the CRC answers
//!   `{"response_token": "sha256=<base64 of HMAC(crc_token)>"}`.
//! - LinkedIn: `X-LI-Signature: <hex>`, HMAC-SHA256 of `hmacsha256=` plus
//!   the body with the client secret; the validation answers
//!   `{"challengeCode", "challengeResponse": <hex of HMAC(code)>}`.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::{HookAnswer, HookRequest};

type HmacSha256 = Hmac<Sha256>;

fn mac(secret: &str, parts: &[&[u8]]) -> Option<HmacSha256> {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).ok()?;
    for part in parts {
        mac.update(part);
    }
    Some(mac)
}

fn digest(secret: &str, parts: &[&[u8]]) -> Vec<u8> {
    mac(secret, parts).map(|mac| mac.finalize().into_bytes().to_vec()).unwrap_or_default()
}

fn verifies(secret: &str, parts: &[&[u8]], claimed: Option<Vec<u8>>) -> bool {
    match (secret.is_empty(), claimed, mac(secret, parts)) {
        (false, Some(claimed), Some(mac)) => mac.verify_slice(&claimed).is_ok(),
        _ => false,
    }
}

pub fn meta(app_secret: &str, request: &HookRequest) -> bool {
    let claimed = request
        .header("x-hub-signature-256")
        .and_then(|value| value.trim().strip_prefix("sha256="))
        .and_then(|hex| hex::decode(hex).ok());
    verifies(app_secret, &[&request.body], claimed)
}

pub fn meta_header(app_secret: &str, body: &[u8]) -> String {
    format!("sha256={}", hex::encode(digest(app_secret, &[body])))
}

pub fn meta_challenge(verify_token: &str, request: &HookRequest) -> HookAnswer {
    let ours = !verify_token.is_empty()
        && request.query("hub.mode") == Some("subscribe")
        && request.query("hub.verify_token").is_some_and(|token| constant_eq(token, verify_token));
    match request.query("hub.challenge") {
        Some(challenge) if ours => HookAnswer::Text(challenge.to_owned()),
        _ => HookAnswer::Refused,
    }
}

pub fn x(consumer_secret: &str, request: &HookRequest) -> bool {
    let claimed = request
        .header("x-twitter-webhooks-signature")
        .and_then(|value| value.trim().strip_prefix("sha256="))
        .and_then(|b64| STANDARD.decode(b64).ok());
    verifies(consumer_secret, &[&request.body], claimed)
}

pub fn x_crc(consumer_secret: &str, request: &HookRequest) -> HookAnswer {
    match request.query("crc_token") {
        Some(token) if !consumer_secret.is_empty() => {
            let response = format!("sha256={}", STANDARD.encode(digest(consumer_secret, &[token.as_bytes()])));
            HookAnswer::Json(serde_json::json!({ "response_token": response }).to_string())
        }
        _ => HookAnswer::Refused,
    }
}

pub fn linkedin(client_secret: &str, request: &HookRequest) -> bool {
    let claimed = request.header("x-li-signature").and_then(|hex| hex::decode(hex.trim()).ok());
    verifies(client_secret, &[b"hmacsha256=", &request.body], claimed)
}

pub fn linkedin_challenge(client_secret: &str, request: &HookRequest) -> HookAnswer {
    match request.query("challengeCode") {
        Some(code) if !client_secret.is_empty() => {
            let response = hex::encode(digest(client_secret, &[code.as_bytes()]));
            HookAnswer::Json(serde_json::json!({ "challengeCode": code, "challengeResponse": response }).to_string())
        }
        _ => HookAnswer::Refused,
    }
}

fn constant_eq(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "app-secret";
    const BODY: &[u8] = br#"{"object":"whatsapp_business_account"}"#;

    fn hex_mac(parts: &[&[u8]]) -> String {
        hex::encode(digest(SECRET, parts))
    }

    #[test]
    fn meta_takes_only_its_own_signature() {
        let signed = HookRequest::post(&[("X-Hub-Signature-256", &format!("sha256={}", hex_mac(&[BODY])))], BODY);
        assert!(meta(SECRET, &signed));
        assert!(!meta("another-secret", &signed));
        assert!(!meta("", &signed), "no secret, no trust");
        let tampered = HookRequest { body: b"{}".to_vec(), ..signed.clone() };
        assert!(!meta(SECRET, &tampered));
        assert!(!meta(SECRET, &HookRequest::post(&[], BODY)), "unsigned");
        assert!(!meta(SECRET, &HookRequest::post(&[("x-hub-signature-256", "sha256=zz")], BODY)));
    }

    #[test]
    fn meta_echoes_the_challenge_only_to_our_verify_token() {
        let check = |token: &str| {
            let request =
                HookRequest::get(&[("hub.mode", "subscribe"), ("hub.verify_token", token), ("hub.challenge", "1158")]);
            meta_challenge("our-token", &request)
        };
        assert_eq!(check("our-token"), HookAnswer::Text("1158".into()));
        assert_eq!(check("guess"), HookAnswer::Refused);
        let empty = HookRequest::get(&[("hub.mode", "subscribe"), ("hub.verify_token", ""), ("hub.challenge", "1")]);
        assert_eq!(meta_challenge("", &empty), HookAnswer::Refused);
    }

    #[test]
    fn x_answers_its_crc_and_checks_its_signature() {
        let answer = x_crc(SECRET, &HookRequest::get(&[("crc_token", "abc")]));
        let expected = format!("sha256={}", STANDARD.encode(digest(SECRET, &[b"abc"])));
        assert_eq!(answer, HookAnswer::Json(serde_json::json!({ "response_token": expected }).to_string()));
        assert_eq!(x_crc(SECRET, &HookRequest::get(&[])), HookAnswer::Refused);

        let signature = format!("sha256={}", STANDARD.encode(digest(SECRET, &[BODY])));
        assert!(x(SECRET, &HookRequest::post(&[("x-twitter-webhooks-signature", &signature)], BODY)));
        assert!(!x("other", &HookRequest::post(&[("x-twitter-webhooks-signature", &signature)], BODY)));
    }

    #[test]
    fn linkedin_signs_the_prefixed_body_and_answers_its_challenge() {
        let signature = hex_mac(&[b"hmacsha256=", BODY]);
        assert!(linkedin(SECRET, &HookRequest::post(&[("X-LI-Signature", &signature)], BODY)));
        assert!(!linkedin(SECRET, &HookRequest::post(&[("X-LI-Signature", &hex_mac(&[BODY]))], BODY)));

        let answer = linkedin_challenge(SECRET, &HookRequest::get(&[("challengeCode", "890e4665")]));
        let HookAnswer::Json(json) = answer else { panic!("expected JSON, got {answer:?}") };
        let json: serde_json::Value = serde_json::from_str(&json).expect("JSON");
        assert_eq!(json["challengeCode"], "890e4665");
        assert_eq!(json["challengeResponse"], hex_mac(&[b"890e4665"]));
    }
}
