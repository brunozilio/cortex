use std::collections::HashMap;
use std::io::Write;

use sha2::{Digest, Sha256};

use super::*;

const MAC: Platform<'static> = Platform { arch: "aarch64", os: "macos", libc: "" };
const ASSET: &str = "superfik-0.4.0-aarch64-apple-darwin.tar.gz";

struct Fake {
    tag: String,
    files: HashMap<String, Vec<u8>>,
}

impl Releases for Fake {
    fn latest_tag(&self) -> Result<String, String> {
        Ok(self.tag.clone())
    }

    fn download(&self, tag: &str, name: &str) -> Result<Vec<u8>, String> {
        assert_eq!(tag, self.tag);
        self.files.get(name).cloned().ok_or_else(|| format!("{name}: 404"))
    }
}

/// An archive laid out as the release job packs it.
fn archive(binary: &[u8]) -> Vec<u8> {
    let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    let mut tar = tar::Builder::new(encoder);
    for (path, data) in [
        ("superfik-0.4.0-aarch64-apple-darwin/README.md", b"readme".as_slice()),
        ("superfik-0.4.0-aarch64-apple-darwin/superfik", binary),
    ] {
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(0o755);
        header.set_cksum();
        tar.append_data(&mut header, path, data).expect("append");
    }
    tar.into_inner().expect("tar").finish().expect("gzip")
}

fn release(binary: &[u8], digest_of: Option<&[u8]>) -> Fake {
    let archive = archive(binary);
    let digest = hex(&Sha256::digest(digest_of.unwrap_or(&archive)));
    let files = HashMap::from([
        (format!("{ASSET}.sha256"), format!("{digest}  {ASSET}\n").into_bytes()),
        (ASSET.to_owned(), archive),
    ]);
    Fake { tag: "v0.4.0".into(), files }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn installed(dir: &tempfile::TempDir) -> std::path::PathBuf {
    let exe = dir.path().join("superfik");
    std::fs::File::create(&exe).and_then(|mut f| f.write_all(b"old")).expect("old binary");
    exe
}

#[test]
fn a_newer_release_replaces_the_binary() {
    let dir = tempfile::tempdir().expect("dir");
    let exe = installed(&dir);
    let updated = update(&release(b"new", None), "0.3.1", &exe, MAC);
    assert_eq!(updated, Ok(Updated::To(Version(0, 4, 0))));
    assert_eq!(std::fs::read(&exe).expect("read"), b"new");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&exe).expect("meta").permissions().mode();
        assert_eq!(mode & 0o111, 0o111, "still executable");
    }
    let left: Vec<_> = std::fs::read_dir(dir.path()).expect("list").collect();
    assert_eq!(left.len(), 1, "nothing left beside it");
}

#[test]
fn an_archive_that_does_not_match_its_checksum_is_not_installed() {
    let dir = tempfile::tempdir().expect("dir");
    let exe = installed(&dir);
    let updated = update(&release(b"tampered", Some(b"something else")), "0.3.1", &exe, MAC);
    assert!(updated.is_err_and(|e| e.contains("checksum")));
    assert_eq!(std::fs::read(&exe).expect("read"), b"old");
}

#[test]
fn the_latest_release_leaves_the_binary_alone() {
    let dir = tempfile::tempdir().expect("dir");
    let exe = installed(&dir);
    assert_eq!(update(&release(b"new", None), "0.4.0", &exe, MAC), Ok(Updated::AlreadyLatest));
    assert_eq!(std::fs::read(&exe).expect("read"), b"old");
}

#[test]
fn a_platform_without_a_build_says_so() {
    let dir = tempfile::tempdir().expect("dir");
    let exe = installed(&dir);
    let intel = Platform { arch: "x86_64", os: "macos", libc: "" };
    assert!(update(&release(b"new", None), "0.3.1", &exe, intel).is_err_and(|e| e.contains("x86_64")));
    assert_eq!(std::fs::read(&exe).expect("read"), b"old");
}
