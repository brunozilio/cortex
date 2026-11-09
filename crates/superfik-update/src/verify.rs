//! The archive's checks: its size and SHA-256 against the manifest, and
//! the Ed25519 signature over that digest made with Superfik's update key
//! (its secret half stays with the maintainers; forks build with theirs).

use std::io::Read;
use std::path::Path;

use ed25519_dalek::{Signature, VerifyingKey};
use sha2::{Digest, Sha256};

use crate::{Asset, UpdateError};

pub const UPDATE_KEY: [u8; 32] = [
    0x44, 0x7a, 0xbf, 0x29, 0x6f, 0x0f, 0x7a, 0xe2, 0x4d, 0x82, 0x40, 0x82, 0x6a, 0x02, 0x9e, 0x02, 0x4d, 0xe2, 0xa8,
    0xf3, 0x2d, 0x9c, 0xd5, 0x55, 0x81, 0xe3, 0x3a, 0x43, 0x54, 0x98, 0xfa, 0x56,
];

pub fn verify(bytes_sha256: &[u8; 32], asset: &Asset, key: &[u8; 32]) -> Result<(), UpdateError> {
    let key = VerifyingKey::from_bytes(key).map_err(|_| UpdateError::Forged)?;
    let raw: [u8; 64] =
        hex::decode(asset.signature.trim()).ok().and_then(|bytes| bytes.try_into().ok()).ok_or(UpdateError::Forged)?;
    key.verify_strict(bytes_sha256, &Signature::from_bytes(&raw)).map_err(|_| UpdateError::Forged)
}

pub fn digest_file(path: &Path) -> std::io::Result<(u64, [u8; 32])> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0; 1 << 16];
    let mut size = 0;
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        size += read as u64;
    }
    Ok((size, hasher.finalize().into()))
}

pub(crate) fn matches(asset: &Asset, size: u64, digest: &[u8; 32]) -> bool {
    size == asset.size && asset.sha256.trim().eq_ignore_ascii_case(&hex::encode(digest))
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer, SigningKey};

    use super::*;

    fn signed(seed: u8, digest: &[u8; 32]) -> (Asset, [u8; 32]) {
        let key = SigningKey::from_bytes(&[seed; 32]);
        let asset = Asset {
            url: "https://example.com/a.zip".into(),
            size: 3,
            sha256: hex::encode(digest),
            signature: hex::encode(key.sign(digest).to_bytes()),
        };
        (asset, key.verifying_key().to_bytes())
    }

    fn digest(bytes: &[u8]) -> [u8; 32] {
        Sha256::digest(bytes).into()
    }

    #[test]
    fn a_good_signature_passes() {
        let (asset, key) = signed(7, &digest(b"abc"));
        assert_eq!(verify(&digest(b"abc"), &asset, &key), Ok(()));
    }

    #[test]
    fn another_digest_is_forged() {
        let (asset, key) = signed(7, &digest(b"abc"));
        assert_eq!(verify(&digest(b"abd"), &asset, &key), Err(UpdateError::Forged));
    }

    #[test]
    fn another_key_is_forged() {
        let (asset, _) = signed(7, &digest(b"abc"));
        let (_, other) = signed(8, &digest(b"abc"));
        assert_eq!(verify(&digest(b"abc"), &asset, &other), Err(UpdateError::Forged));
    }

    #[test]
    fn an_unreadable_signature_is_forged() {
        let (mut asset, key) = signed(7, &digest(b"abc"));
        asset.signature = "zz".into();
        assert_eq!(verify(&digest(b"abc"), &asset, &key), Err(UpdateError::Forged));
    }

    #[test]
    fn the_built_in_key_is_a_real_key() {
        assert!(VerifyingKey::from_bytes(&UPDATE_KEY).is_ok());
        assert_eq!(hex::encode(UPDATE_KEY), "447abf296f0f7ae24d8240826a029e024de2a8f32d9cd55581e33a435498fa56");
    }

    #[test]
    fn digests_a_file_and_matches_the_manifest() {
        let dir = tempfile::tempdir().unwrap_or_else(|error| panic!("{error}"));
        let path = dir.path().join("a");
        std::fs::write(&path, b"abc").unwrap_or_else(|error| panic!("{error}"));
        let (size, hash) = digest_file(&path).unwrap_or_else(|error| panic!("{error}"));
        let (asset, _) = signed(1, &digest(b"abc"));
        assert_eq!((size, hash), (3, digest(b"abc")));
        assert!(matches(&asset, size, &hash));
        assert!(!matches(&Asset { size: 4, ..asset.clone() }, size, &hash));
        assert!(!matches(&Asset { sha256: hex::encode(digest(b"x")), ..asset }, size, &hash));
    }
}
