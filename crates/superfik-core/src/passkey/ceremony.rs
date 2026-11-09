//! The bytes of a WebAuthn ceremony as an authenticator writes them: the
//! client data, the authenticator data, the COSE key of a new passkey and
//! its `none` attestation (hand-written CBOR: the shapes never vary).

use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ceremony {
    Create,
    Get,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PublicKey {
    pub x: [u8; 32],
    pub y: [u8; 32],
}

const FLAGS: u8 = 0x01 | 0x04 | 0x08 | 0x10;
const ATTESTED: u8 = 0x40;

pub fn client_data_json(ceremony: Ceremony, challenge: &str, origin: &str) -> String {
    let kind = match ceremony {
        Ceremony::Create => "webauthn.create",
        Ceremony::Get => "webauthn.get",
    };
    let text = |s: &str| serde_json::Value::from(s).to_string();
    format!(r#"{{"type":"{kind}","challenge":{},"origin":{},"crossOrigin":false}}"#, text(challenge), text(origin))
}

pub fn authenticator_data(rp_id: &str, counter: u32, attested: Option<(&[u8], &PublicKey)>) -> Vec<u8> {
    let mut data = Sha256::digest(rp_id.as_bytes()).to_vec();
    data.push(if attested.is_some() { FLAGS | ATTESTED } else { FLAGS });
    data.extend(counter.to_be_bytes());
    if let Some((id, key)) = attested {
        data.extend([0; 16]);
        data.extend(u16::try_from(id.len()).unwrap_or(u16::MAX).to_be_bytes());
        data.extend(id);
        data.extend(cose_key(key));
    }
    data
}

fn cose_key(key: &PublicKey) -> Vec<u8> {
    let mut cose = vec![0xa5, 0x01, 0x02, 0x03, 0x26, 0x20, 0x01, 0x21, 0x58, 0x20];
    cose.extend(key.x);
    cose.extend([0x22, 0x58, 0x20]);
    cose.extend(key.y);
    cose
}

pub fn attestation_object(auth_data: &[u8]) -> Vec<u8> {
    let mut object = vec![0xa3, 0x63];
    object.extend(b"fmt");
    object.push(0x64);
    object.extend(b"none");
    object.push(0x67);
    object.extend(b"attStmt");
    object.extend([0xa0, 0x68]);
    object.extend(b"authData");
    object.extend(byte_string_head(auth_data.len()));
    object.extend(auth_data);
    object
}

fn byte_string_head(len: usize) -> Vec<u8> {
    match len {
        0..24 => vec![0x40 | len as u8],
        24..256 => vec![0x58, len as u8],
        _ => {
            let [hi, lo] = u16::try_from(len).unwrap_or(u16::MAX).to_be_bytes();
            vec![0x59, hi, lo]
        }
    }
}

pub fn credential_bytes(credential: &str) -> Option<Vec<u8>> {
    if let Some(encoded) = credential.strip_prefix("b64.") {
        return decode(encoded);
    }
    uuid::Uuid::parse_str(credential).ok().map(|id| id.as_bytes().to_vec())
}

pub fn credential_id(credential: &str) -> Option<String> {
    credential_bytes(credential).map(|bytes| URL_SAFE_NO_PAD.encode(bytes))
}

pub fn decode(text: &str) -> Option<Vec<u8>> {
    let trimmed = text.trim().trim_end_matches('=');
    URL_SAFE_NO_PAD.decode(trimmed).ok().or_else(|| STANDARD.decode(text.trim()).ok())
}
