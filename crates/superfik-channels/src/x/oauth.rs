//! OAuth 1.0a user context (RFC 5849, HMAC-SHA1), which X's v2 endpoints
//! for DMs, mentions and posts accept and whose tokens never expire.
//! Pure: the nonce and time are passed in.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use hmac::{Hmac, Mac};
use sha1::Sha1;

#[derive(Clone)]
pub struct Keys {
    pub consumer_key: String,
    pub consumer_secret: String,
    pub token: String,
    pub token_secret: String,
}

impl std::fmt::Debug for Keys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Keys").field("consumer_key", &self.consumer_key).finish_non_exhaustive()
    }
}

use crate::http::encode;

pub fn header(keys: &Keys, method: &str, url: &str, extra: &[(&str, &str)], nonce: &str, timestamp: u64) -> String {
    let timestamp = timestamp.to_string();
    let oauth = [
        ("oauth_consumer_key", keys.consumer_key.as_str()),
        ("oauth_nonce", nonce),
        ("oauth_signature_method", "HMAC-SHA1"),
        ("oauth_timestamp", timestamp.as_str()),
        ("oauth_token", keys.token.as_str()),
        ("oauth_version", "1.0"),
    ];
    let (base, query) = url.split_once('?').unwrap_or((url, ""));
    let from_query = query.split('&').filter(|pair| !pair.is_empty()).map(|pair| {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        (decode(k), decode(v))
    });
    let mut params: Vec<(String, String)> = oauth
        .iter()
        .chain(extra)
        .map(|(k, v)| (encode(k), encode(v)))
        .chain(from_query.map(|(k, v)| (encode(&k), encode(&v))))
        .collect();
    params.sort();
    let joined: Vec<String> = params.iter().map(|(k, v)| format!("{k}={v}")).collect();
    let text = format!("{}&{}&{}", method.to_ascii_uppercase(), encode(base), encode(&joined.join("&")));
    let key = format!("{}&{}", encode(&keys.consumer_secret), encode(&keys.token_secret));
    let signature = Hmac::<Sha1>::new_from_slice(key.as_bytes())
        .map(|mut mac| {
            mac.update(text.as_bytes());
            STANDARD.encode(mac.finalize().into_bytes())
        })
        .unwrap_or_default();
    let fields: Vec<String> = oauth
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .chain([("oauth_signature".to_owned(), signature)])
        .map(|(k, v)| format!("{k}=\"{}\"", encode(&v)))
        .collect();
    format!("OAuth {}", fields.join(", "))
}

fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
                match hex.and_then(|hex| u8::from_str_radix(hex, 16).ok()) {
                    Some(byte) => out.push(byte),
                    None => out.extend(&bytes[i..i + 3]),
                }
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            byte => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signs_xs_documented_example() {
        let keys = Keys {
            consumer_key: "xvz1evFS4wEEPTGEFPHBog".into(),
            consumer_secret: "kAcSOqF21Fu85e7zjz7ZN2U4ZRhfV3WpwPAoE3Z7kBw".into(),
            token: "370773112-GmHxMAgYyLbNEtIKZeRNFsMKPR9EyMZeS9weJAEb".into(),
            token_secret: "LswwdoUaIvS8ltyTt5jkRh4J50vUPVVHtR2YPi5kE".into(),
        };
        let header = header(
            &keys,
            "post",
            "https://api.twitter.com/1.1/statuses/update.json?include_entities=true",
            &[("status", "Hello Ladies + Gentlemen, a signed OAuth request!")],
            "kYjzVBB8Y0ZFabxSWbWovY3uYSQ2pTgmZeNu2VS4cg",
            1_318_622_958,
        );
        assert!(header.contains("oauth_signature=\"hCtSmYh%2BiHYCEqBWrE7C7hYmtUk%3D\""), "{header}");
        assert!(header.starts_with("OAuth oauth_consumer_key=\"xvz1evFS4wEEPTGEFPHBog\""));
    }

    #[test]
    fn encoding_keeps_only_unreserved_characters() {
        assert_eq!(encode("Ladies + Gentlemen"), "Ladies%20%2B%20Gentlemen");
        assert_eq!(encode("a-b.c_d~é"), "a-b.c_d~%C3%A9");
        assert_eq!(decode("a%20b+c%2B"), "a b c+");
    }
}
