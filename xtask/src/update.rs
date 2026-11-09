//! The update key: `update-keygen <out-file>` makes the Ed25519 key pair
//! (the secret seed, hex, written `0600`; the public key printed for
//! `superfik_update::UPDATE_KEY`), and `sign-update <archive> --version
//! X.Y.Z --platform P --url URL` signs a release archive with the seed in
//! `SUPERFIK_UPDATE_KEY`, printing the manifest's asset JSON.

use std::path::Path;

use ed25519_dalek::{Signer, SigningKey};
use superfik_update::{Asset, Platform, Version};

type Error = Box<dyn std::error::Error>;

pub fn keygen(out: &Path) -> Result<(), Error> {
    if out.exists() {
        return Err(format!("{} exists; not replacing a signing key", out.display()).into());
    }
    let mut seed = [0u8; 32];
    getrandom::fill(&mut seed).map_err(|error| format!("no randomness: {error}"))?;
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)?;
    }
    superfik_private_file::write(out, format!("{}\n", hex::encode(seed)).as_bytes())?;
    println!("{}", hex::encode(SigningKey::from_bytes(&seed).verifying_key().to_bytes()));
    Ok(())
}

pub fn sign(args: &[String]) -> Result<(), Error> {
    let usage = "usage: xtask sign-update <archive> --version X.Y.Z --platform P --url URL";
    let [archive, rest @ ..] = args else { return Err(usage.into()) };
    let flag = |name: &str| {
        rest.windows(2).find(|pair| pair[0] == name).map(|pair| pair[1].clone()).ok_or_else(|| usage.to_owned())
    };
    let version: Version = flag("--version")?.parse()?;
    let platform: Platform = flag("--platform")?.parse()?;
    let url = flag("--url")?;
    if !url.contains(&version.to_string()) {
        return Err(format!("{url} does not name version {version}").into());
    }
    let seed = std::env::var("SUPERFIK_UPDATE_KEY").map_err(|_| "SUPERFIK_UPDATE_KEY is not set")?;
    let key = signing_key(&seed)?;
    if key.verifying_key().to_bytes() != superfik_update::UPDATE_KEY {
        return Err("SUPERFIK_UPDATE_KEY is not the key of superfik_update::UPDATE_KEY".into());
    }
    let (size, digest) = superfik_update::digest_file(Path::new(archive))?;
    let asset = asset(&key, url, size, &digest);
    superfik_update::verify(&digest, &asset, &superfik_update::UPDATE_KEY)?;
    eprintln!("signed {archive} for {platform}");
    println!("{}", serde_json::to_string(&asset)?);
    Ok(())
}

pub fn signing_key(seed_hex: &str) -> Result<SigningKey, Error> {
    let seed: [u8; 32] = hex::decode(seed_hex.trim())
        .ok()
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or("SUPERFIK_UPDATE_KEY is not 64 hex digits")?;
    Ok(SigningKey::from_bytes(&seed))
}

pub fn asset(key: &SigningKey, url: String, size: u64, digest: &[u8; 32]) -> Asset {
    Asset { url, size, sha256: hex::encode(digest), signature: hex::encode(key.sign(digest).to_bytes()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_signed_archive_verifies_with_the_public_key() {
        let dir = tempfile::tempdir().expect("temp dir");
        let archive = dir.path().join("Superfik-1.2.3-macos-aarch64.zip");
        std::fs::write(&archive, b"archive bytes").expect("write");
        let key = signing_key(&format!(" {}\n", hex::encode([5u8; 32]))).expect("key");
        let (size, digest) = superfik_update::digest_file(&archive).expect("digest");
        let asset = asset(&key, "https://example.com/x.zip".into(), size, &digest);
        let public = key.verifying_key().to_bytes();
        assert_eq!(asset.size, 13);
        assert_eq!(asset.sha256.len(), 64);
        assert_eq!(asset.signature.len(), 128);
        assert_eq!(superfik_update::verify(&digest, &asset, &public), Ok(()));
        let mut other = digest;
        other[0] ^= 1;
        assert!(superfik_update::verify(&other, &asset, &public).is_err());
    }

    #[test]
    fn keygen_writes_a_private_seed_and_never_replaces_one() {
        let dir = tempfile::tempdir().expect("temp dir");
        let out = dir.path().join("keys/update.hex");
        keygen(&out).expect("keygen");
        let seed = std::fs::read_to_string(&out).expect("read");
        assert!(signing_key(&seed).is_ok());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&out).expect("metadata").permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        assert!(keygen(&out).is_err());
        assert_eq!(std::fs::read_to_string(&out).expect("read"), seed);
    }

    #[test]
    fn a_short_seed_is_refused() {
        assert!(signing_key("abcd").is_err());
    }
}
