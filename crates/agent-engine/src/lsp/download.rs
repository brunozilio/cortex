use std::fs::File;
use std::io;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use domain::lsp::install::Archive;
use sha2::{Digest, Sha256};

use crate::model::http::{Http, NetHttp};

const QUIET: Duration = Duration::from_secs(60);

pub(super) struct Download<'a> {
    pub url: &'a str,
    pub archive: Archive,
    pub sha256: &'a str,
}

impl Download<'_> {
    pub(super) fn unpack_into(&self, staging: &Path, program: &Path) -> Result<(), String> {
        let never = AtomicBool::new(false);
        let response = NetHttp.get_stream(self.url, &[], &never).map_err(|error| error.to_string())?;
        if response.status != 200 {
            return Err(format!("{} answered {}", self.url, response.status));
        }
        unpack(Patient::new(response.body, QUIET), self.archive, self.sha256, staging, program)
            .map_err(|error| format!("{}: {error}", self.url))
    }
}

fn unpack(body: impl io::Read, archive: Archive, sha256: &str, staging: &Path, program: &Path) -> Result<(), String> {
    let saved = |error: io::Error| format!("cannot save the download: {error}");
    let mut body = Hashed { inner: body, hash: Sha256::new() };
    match archive {
        Archive::Gzip => {
            let mut file = File::create(program).map_err(saved)?;
            io::copy(&mut flate2::read::GzDecoder::new(&mut body), &mut file).map_err(saved)?;
            drop(file);
            if let Err(error) = body.matches(sha256) {
                std::fs::remove_file(program).map_err(saved)?;
                return Err(error);
            }
        }
        Archive::Zip => {
            let path = staging.join(".download.zip");
            io::copy(&mut body, &mut File::create(&path).map_err(saved)?).map_err(saved)?;
            let unpacked = body.matches(sha256).and_then(|()| {
                zip::ZipArchive::new(File::open(&path).map_err(saved)?)
                    .map_err(|error| format!("not a zip: {error}"))?
                    .extract(staging)
                    .map_err(|error| format!("cannot unpack: {error}"))
            });
            std::fs::remove_file(&path).map_err(saved)?;
            unpacked?;
        }
    }
    Ok(())
}

struct Hashed<R> {
    inner: R,
    hash: Sha256,
}

impl<R: io::Read> Hashed<R> {
    fn matches(mut self, sha256: &str) -> Result<(), String> {
        io::copy(&mut self, &mut io::sink()).map_err(|error| format!("cannot read the download: {error}"))?;
        let found = format!("{:x}", self.hash.finalize());
        if found == sha256 {
            Ok(())
        } else {
            Err(format!("its checksum is {found}, not the expected {sha256}; nothing was installed"))
        }
    }
}

impl<R: io::Read> io::Read for Hashed<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let read = self.inner.read(buf)?;
        self.hash.update(&buf[..read]);
        Ok(read)
    }
}

struct Patient<R> {
    inner: R,
    quiet: Duration,
}

impl<R: io::Read> Patient<R> {
    const fn new(inner: R, quiet: Duration) -> Self {
        Self { inner, quiet }
    }
}

impl<R: io::Read> io::Read for Patient<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let started = std::time::Instant::now();
        loop {
            match self.inner.read(buf) {
                Err(error) if error.kind() == io::ErrorKind::TimedOut && started.elapsed() < self.quiet => {}
                other => return other,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::io::Read;

    use super::*;

    const ANOTHER: &str = "0000000000000000000000000000000000000000000000000000000000000000";

    fn sum(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    fn gzip(content: &[u8]) -> Vec<u8> {
        let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        io::Write::write_all(&mut gzip, content).expect("gzip");
        gzip.finish().expect("gzip")
    }

    fn zip_of(name: &str, content: &[u8]) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(io::Cursor::new(Vec::new()));
        zip.start_file(name, zip::write::SimpleFileOptions::default()).expect("entry");
        io::Write::write_all(&mut zip, content).expect("zip");
        zip.finish().expect("zip").into_inner()
    }

    #[test]
    fn a_checksum_is_of_the_bytes_downloaded() {
        let hashed = Hashed { inner: &b"abc"[..], hash: Sha256::new() };
        hashed.matches("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad").expect("known digest");
    }

    #[test]
    fn a_gzip_is_the_program_and_a_zip_unpacks_into_the_folder() {
        let staging = tempfile::tempdir().expect("staging");
        let bytes = gzip(b"#!bin");
        let program = staging.path().join("rust-analyzer");
        unpack(&bytes[..], Archive::Gzip, &sum(&bytes), staging.path(), &program).expect("gunzip");
        assert_eq!(std::fs::read(&program).expect("program"), b"#!bin");
        let bytes = zip_of("clangd_1/bin/clangd", b"elf");
        let program = staging.path().join("clangd_1/bin/clangd");
        unpack(&bytes[..], Archive::Zip, &sum(&bytes), staging.path(), &program).expect("unzip");
        assert_eq!(std::fs::read(&program).expect("program"), b"elf");
        assert!(!staging.path().join(".download.zip").exists());
        assert!(unpack(&b"not a zip"[..], Archive::Zip, &sum(b"not a zip"), staging.path(), &program).is_err());
    }

    #[test]
    fn a_download_with_another_checksum_is_not_installed() {
        let staging = tempfile::tempdir().expect("staging");
        let program = staging.path().join("rust-analyzer");
        let refused = unpack(&gzip(b"#!bin")[..], Archive::Gzip, ANOTHER, staging.path(), &program).expect_err("gzip");
        assert!(refused.contains("checksum") && refused.contains(ANOTHER), "{refused}");
        assert!(!program.exists());
        let bytes = zip_of("clangd_1/bin/clangd", b"elf");
        unpack(&bytes[..], Archive::Zip, ANOTHER, staging.path(), &program).expect_err("zip");
        assert_eq!(std::fs::read_dir(staging.path()).expect("list").count(), 0);
    }

    struct Stalling(VecDeque<io::Result<Vec<u8>>>);

    impl Read for Stalling {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            match self.0.pop_front() {
                Some(Ok(bytes)) => {
                    buf[..bytes.len()].copy_from_slice(&bytes);
                    Ok(bytes.len())
                }
                Some(Err(error)) => Err(error),
                None => Ok(0),
            }
        }
    }

    fn pause() -> io::Result<Vec<u8>> {
        Err(io::Error::new(io::ErrorKind::TimedOut, "waiting for stream"))
    }

    #[test]
    fn a_download_waits_through_quiet_moments_but_not_forever() {
        let mut read = Vec::new();
        let stalling = Stalling([pause(), Ok(b"ab".to_vec()), pause(), Ok(b"c".to_vec())].into());
        Patient::new(stalling, Duration::from_secs(5)).read_to_end(&mut read).expect("read");
        assert_eq!(read, b"abc");
        let silent = Stalling(std::iter::repeat_with(pause).take(1000).collect());
        let error = Patient::new(silent, Duration::ZERO).read_to_end(&mut Vec::new()).expect_err("gave up");
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    }
}
