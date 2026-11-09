//! Chromium's cookie store: where it is, and how its values are
//! decrypted: AES-128-CBC with a key derived from a secret (macOS: the
//! Keychain's, `v10`; Linux: the Secret Service's for `v11`, a fixed one
//! for `v10`). Pure, apart from [`rows`] reading the database.

use std::path::{Path, PathBuf};

use aes::cipher::{BlockDecryptMut, KeyIvInit, block_padding::Pkcs7};

use super::{Cookie, SameSite};

const EPOCH_GAP: f64 = 11_644_473_600.0;
const HASHED_FROM: i64 = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Keys {
    pub v10: [u8; 16],
    pub v11: Option<[u8; 16]>,
}

impl Keys {
    pub fn mac(secret: &str) -> Self {
        Self { v10: derive_key(secret, 1003), v11: None }
    }

    pub fn linux(secret: Option<&str>) -> Self {
        Self { v10: derive_key("peanuts", 1), v11: secret.map(|secret| derive_key(secret, 1)) }
    }
}

pub fn derive_key(secret: &str, rounds: u32) -> [u8; 16] {
    let mut key = [0u8; 16];
    pbkdf2::pbkdf2_hmac::<sha1::Sha1>(secret.as_bytes(), b"saltysalt", rounds, &mut key);
    key
}

pub fn decrypt(encrypted: &[u8], keys: &Keys, hashed: bool) -> Option<String> {
    let (key, body) = match encrypted.split_at_checked(3)? {
        (b"v10", body) => (keys.v10, body),
        (b"v11", body) => (keys.v11?, body),
        _ => return None,
    };
    let iv = [b' '; 16];
    let mut buffer = body.to_vec();
    let plain =
        cbc::Decryptor::<aes::Aes128>::new(&key.into(), &iv.into()).decrypt_padded_mut::<Pkcs7>(&mut buffer).ok()?;
    let plain = if hashed { plain.get(32..)? } else { plain };
    String::from_utf8(plain.to_vec()).ok()
}

pub fn cookie_db(dir: &Path) -> Option<PathBuf> {
    let last = std::fs::read_to_string(dir.join("Local State"))
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|state| state["profile"]["last_used"].as_str().map(str::to_owned));
    let profiles = last.into_iter().chain(["Default".to_owned()]);
    profiles
        .flat_map(|profile| [dir.join(&profile).join("Network/Cookies"), dir.join(&profile).join("Cookies")])
        .find(|p| p.exists())
}

#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub host: String,
    pub name: String,
    pub value: String,
    pub encrypted: Vec<u8>,
    pub path: String,
    pub expires: i64,
    pub secure: bool,
    pub http_only: bool,
    pub same_site: i64,
}

impl Row {
    pub fn cookie(&self, keys: &Keys, hashed: bool) -> Option<Cookie> {
        let value =
            if self.encrypted.is_empty() { self.value.clone() } else { decrypt(&self.encrypted, keys, hashed)? };
        Some(Cookie {
            name: self.name.clone(),
            value,
            domain: self.host.clone(),
            path: self.path.clone(),
            secure: self.secure,
            http_only: self.http_only,
            same_site: match self.same_site {
                0 => Some(SameSite::None),
                1 => Some(SameSite::Lax),
                2 => Some(SameSite::Strict),
                _ => None,
            },
            expires: (self.expires > 0).then(|| self.expires as f64 / 1_000_000.0 - EPOCH_GAP),
        })
    }
}

pub fn rows(path: &Path) -> rusqlite::Result<(Vec<Row>, bool)> {
    let db = rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let version: i64 = db
        .query_row("SELECT value FROM meta WHERE key = 'version'", [], |r| r.get::<_, String>(0))
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let mut statement = db.prepare(
        "SELECT host_key, name, value, encrypted_value, path, expires_utc, is_secure, is_httponly, samesite FROM cookies",
    )?;
    let rows = statement
        .query_map([], |r| {
            Ok(Row {
                host: r.get(0)?,
                name: r.get(1)?,
                value: r.get(2)?,
                encrypted: r.get(3)?,
                path: r.get(4)?,
                expires: r.get(5)?,
                secure: r.get::<_, i64>(6)? != 0,
                http_only: r.get::<_, i64>(7)? != 0,
                same_site: r.get(8)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<Row>>>()?;
    Ok((rows, version >= HASHED_FROM))
}

#[cfg(test)]
mod tests {
    use aes::cipher::BlockEncryptMut;

    use super::*;

    fn encrypt(prefix: &[u8], plain: &[u8], key: &[u8; 16]) -> Vec<u8> {
        let iv = [b' '; 16];
        let mut out = prefix.to_vec();
        out.extend(cbc::Encryptor::<aes::Aes128>::new(key.into(), &iv.into()).encrypt_padded_vec_mut::<Pkcs7>(plain));
        out
    }

    #[test]
    fn values_decrypt_with_and_without_the_host_hash() {
        let keys = Keys::mac("peanuts");
        assert_eq!(decrypt(&encrypt(b"v10", b"token123", &keys.v10), &keys, false).as_deref(), Some("token123"));
        let mut hashed = vec![0u8; 32];
        hashed.extend(b"token123");
        assert_eq!(decrypt(&encrypt(b"v10", &hashed, &keys.v10), &keys, true).as_deref(), Some("token123"));
        let other = Keys::mac("other");
        assert_eq!(decrypt(&encrypt(b"v10", b"token123", &other.v10), &keys, false), None, "wrong key");
        assert_eq!(decrypt(b"plain", &keys, false), None, "not encrypted");
    }

    #[test]
    fn linux_reads_v10_with_the_fixed_password_and_v11_with_the_keyrings() {
        let keys = Keys::linux(Some("from-the-keyring"));
        assert_eq!(keys.v10, derive_key("peanuts", 1));
        let v11 = derive_key("from-the-keyring", 1);
        assert_eq!(decrypt(&encrypt(b"v10", b"basic", &keys.v10), &keys, false).as_deref(), Some("basic"));
        assert_eq!(decrypt(&encrypt(b"v11", b"keyring", &v11), &keys, false).as_deref(), Some("keyring"));
        let without = Keys::linux(None);
        assert_eq!(decrypt(&encrypt(b"v11", b"keyring", &v11), &without, false), None, "no keyring, no v11");
        assert_eq!(decrypt(&encrypt(b"v11", b"x", &v11), &Keys::mac("peanuts"), false), None, "macOS has no v11");
    }

    #[test]
    fn rows_become_cookies_with_unix_expiry() {
        let keys = Keys::mac("k");
        let row = Row {
            host: ".github.com".into(),
            name: "user_session".into(),
            value: String::new(),
            encrypted: encrypt(b"v10", b"abc", &keys.v10),
            path: "/".into(),
            expires: ((EPOCH_GAP + 1_700_000_000.0) * 1_000_000.0) as i64,
            secure: true,
            http_only: true,
            same_site: 1,
        };
        let cookie = row.cookie(&keys, false).expect("cookie");
        assert_eq!((cookie.value.as_str(), cookie.same_site), ("abc", Some(SameSite::Lax)));
        assert_eq!(cookie.expires.map(|e| e.round()), Some(1_700_000_000.0));
    }

    #[test]
    fn the_last_used_profile_is_read_first() -> std::io::Result<()> {
        let dir = tempfile::tempdir()?;
        std::fs::create_dir_all(dir.path().join("Profile 2/Network"))?;
        std::fs::write(dir.path().join("Profile 2/Network/Cookies"), b"")?;
        std::fs::write(dir.path().join("Local State"), r#"{"profile":{"last_used":"Profile 2"}}"#)?;
        assert_eq!(cookie_db(dir.path()), Some(dir.path().join("Profile 2/Network/Cookies")));
        Ok(())
    }
}
