//! `uv`, which installs `mlx-lm`: the system's when it has one, else a
//! release pinned here, downloaded once from Astral and checked against its
//! SHA-256 before it runs.

use std::path::{Path, PathBuf};

const VERSION: &str = "0.13.5";
const ARCHIVE: &str = "uv-aarch64-apple-darwin";
const SHA256: &str = "743f85035a1996ecb350659e01cd9faea9709829be09b1b4b03c73eaf3c9cd73";
const MAX_ARCHIVE: u64 = 64 << 20;

pub(super) fn ensure(tools: &Path) -> Result<PathBuf, String> {
    if let Some(found) = on_system() {
        return Ok(found);
    }
    let kept = kept(tools);
    if kept.is_file() {
        return Ok(kept);
    }
    tracing::info!(version = VERSION, "downloading uv");
    let archive = fetch(&format!("https://github.com/astral-sh/uv/releases/download/{VERSION}/{ARCHIVE}.tar.gz"))?;
    unpack(&archive, SHA256, tools)?;
    Ok(kept)
}

fn kept(tools: &Path) -> PathBuf {
    tools.join(format!("uv-{VERSION}")).join("uv")
}

fn on_system() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let mut dirs: Vec<PathBuf> =
        std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    dirs.extend(["/opt/homebrew/bin", "/usr/local/bin"].map(PathBuf::from));
    dirs.extend(home.into_iter().flat_map(|home| [home.join(".local/bin"), home.join(".cargo/bin")]));
    superfik_core::executable::find(std::env::consts::OS, &["uv"], &dirs, Path::is_file)
}

fn fetch(url: &str) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let fail = |error: &dyn std::fmt::Display| format!("Could not download uv: {error}");
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| fail(&e))?;
    let response = client.get(url).send().and_then(|r| r.error_for_status()).map_err(|e| fail(&e))?;
    let mut archive = Vec::new();
    response.take(MAX_ARCHIVE + 1).read_to_end(&mut archive).map_err(|e| fail(&e))?;
    if archive.len() as u64 > MAX_ARCHIVE {
        return Err(fail(&"the archive is too large"));
    }
    Ok(archive)
}

fn unpack(archive: &[u8], expected: &str, tools: &Path) -> Result<(), String> {
    if sha256(archive) != expected {
        return Err("the uv download does not match its checksum".into());
    }
    let staging = tools.join(format!(".uv-{VERSION}-{}", std::process::id()));
    let unpacked = (|| {
        let _ = std::fs::remove_dir_all(&staging);
        std::fs::create_dir_all(&staging).map_err(|error| error.to_string())?;
        let file = staging.join("uv.tar.gz");
        std::fs::write(&file, archive).map_err(|error| error.to_string())?;
        let output = superfik_process::command("tar")
            .arg("-xzf")
            .arg(&file)
            .arg("-C")
            .arg(&staging)
            .output()
            .map_err(|error| error.to_string())?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
        }
        let folder = staging.join(ARCHIVE);
        if !folder.join("uv").is_file() {
            return Err("the uv download has no uv in it".into());
        }
        let dest = kept(tools);
        let dest = dest.parent().unwrap_or(tools);
        let _ = std::fs::remove_dir_all(dest);
        std::fs::rename(&folder, dest).map_err(|error| error.to_string())
    })();
    let _ = std::fs::remove_dir_all(&staging);
    unpacked.map_err(|why| format!("Could not unpack uv: {why}"))
}

fn sha256(bytes: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn archive() -> Vec<u8> {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::create_dir(dir.path().join(ARCHIVE)).expect("folder");
        std::fs::write(dir.path().join(ARCHIVE).join("uv"), "#!/bin/sh\n").expect("uv");
        let file = dir.path().join("uv.tar.gz");
        let made = std::process::Command::new("tar")
            .arg("-czf")
            .arg(&file)
            .arg("-C")
            .arg(dir.path())
            .arg(ARCHIVE)
            .status()
            .expect("tar");
        assert!(made.success());
        std::fs::read(file).expect("archive")
    }

    #[test]
    fn a_checked_archive_is_kept_under_its_version() {
        let tools = tempfile::tempdir().expect("tools");
        let archive = archive();
        unpack(&archive, &sha256(&archive), tools.path()).expect("unpack");
        assert!(kept(tools.path()).is_file());
        let left: Vec<_> = std::fs::read_dir(tools.path())
            .expect("tools")
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(left, [format!("uv-{VERSION}")], "no staging is left behind");
    }

    #[test]
    fn an_archive_that_does_not_match_its_checksum_is_refused() {
        let tools = tempfile::tempdir().expect("tools");
        let refused = unpack(&archive(), SHA256, tools.path()).expect_err("refused");
        assert!(refused.contains("checksum"), "{refused}");
        assert!(!kept(tools.path()).exists());
        assert_eq!(std::fs::read_dir(tools.path()).expect("tools").count(), 0, "nothing is written");
    }
}
