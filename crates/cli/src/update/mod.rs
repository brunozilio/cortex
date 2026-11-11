//! `{command} update`: replaces this binary with the latest release's build
//! for this platform, checked against its published SHA-256. Which
//! release and which archive is `domain::release`; the download is
//! behind [`Releases`] so the replacement is testable.

mod github;

use std::io::{Read, Write};
use std::path::Path;

use domain::release::{self, Version};
use sha2::{Digest, Sha256};

pub use self::github::GitHub;

/// Where releases are published.
pub trait Releases {
    /// The latest release's tag (`v0.3.1`).
    fn latest_tag(&self) -> Result<String, String>;
    /// One file attached to the release tagged `tag`.
    fn download(&self, tag: &str, name: &str) -> Result<Vec<u8>, String>;
}

/// The platform the binary runs on (`std::env::consts::ARCH`, `OS`, and
/// the C library it was built for: `musl` for the static Linux build).
#[derive(Debug, Clone, Copy)]
pub struct Platform<'a> {
    pub arch: &'a str,
    pub os: &'a str,
    pub libc: &'a str,
}

impl Platform<'static> {
    pub fn this() -> Self {
        let libc = if cfg!(target_env = "musl") { "musl" } else { "gnu" };
        Self { arch: std::env::consts::ARCH, os: std::env::consts::OS, libc }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Updated {
    /// `current` is already the latest release.
    AlreadyLatest,
    /// The binary at the path is now this release.
    To(Version),
}

/// `{command} update`: this binary, from GitHub.
pub fn run() -> Result<(), String> {
    let current = env!("CARGO_PKG_VERSION");
    let exe = std::env::current_exe().and_then(std::fs::canonicalize).map_err(|e| e.to_string())?;
    match update(&GitHub::new()?, current, &exe, Platform::this())? {
        Updated::AlreadyLatest => {
            println!("{brand_command} {current} is the latest release", brand_command = brand::COMMAND)
        }
        Updated::To(version) => {
            println!(
                "{brand_command} updated {current} → {version} ({})",
                exe.display(),
                brand_command = brand::COMMAND
            );
            #[cfg(unix)]
            if sdk::daemon::daemon_info().is_some() {
                println!("The background daemon keeps {current} until its last session closes.");
            }
        }
    }
    Ok(())
}

/// Replaces the binary at `exe` (version `current`) with the latest
/// release, unless it is not newer. The file is untouched on any error.
pub fn update(releases: &dyn Releases, current: &str, exe: &Path, platform: Platform<'_>) -> Result<Updated, String> {
    let tag = releases.latest_tag()?;
    let Some(version) = release::newer(current, &tag) else {
        return Ok(Updated::AlreadyLatest);
    };
    let asset = release::asset(version, platform.arch, platform.os, platform.libc).ok_or_else(|| {
        format!(
            "no build of {brand_command} is published for {} {}",
            platform.arch,
            platform.os,
            brand_command = brand::COMMAND
        )
    })?;
    let sums = String::from_utf8_lossy(&releases.download(&tag, &format!("{asset}.sha256"))?).into_owned();
    let expected = release::checksum(&sums, &asset).ok_or_else(|| format!("{asset}.sha256 gives no checksum"))?;
    let archive = releases.download(&tag, &asset)?;
    let actual: String = Sha256::digest(&archive).iter().map(|b| format!("{b:02x}")).collect();
    if actual != expected {
        return Err(format!("{asset} does not match its checksum; nothing was changed"));
    }
    replace(exe, &binary_in(&archive)?)?;
    Ok(Updated::To(version))
}

/// The `{command}` file inside a release archive.
fn binary_in(archive: &[u8]) -> Result<Vec<u8>, String> {
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(archive));
    for entry in tar.entries().map_err(|e| e.to_string())? {
        let mut entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path().map_err(|e| e.to_string())?;
        if path.file_name().is_some_and(|name| name == brand::COMMAND) {
            let mut binary = Vec::new();
            entry.read_to_end(&mut binary).map_err(|e| e.to_string())?;
            return Ok(binary);
        }
    }
    Err(concat!("the release archive has no ", brand::command!(), " binary").into())
}

/// Writes the new binary beside `exe` and renames it over, so `exe` is
/// either the old file or the whole new one.
fn replace(exe: &Path, binary: &[u8]) -> Result<(), String> {
    let name = exe.file_name().ok_or("the binary has no file name")?.to_string_lossy();
    let next = exe.with_file_name(format!(".{name}.update-{}", std::process::id()));
    let written = write_executable(&next, binary).and_then(|()| std::fs::rename(&next, exe));
    if let Err(error) = written {
        let _ = std::fs::remove_file(&next);
        return Err(format!("{}: {error}", exe.display()));
    }
    Ok(())
}

fn write_executable(path: &Path, binary: &[u8]) -> std::io::Result<()> {
    let mut file = std::fs::File::create(path)?;
    file.write_all(binary)?;
    file.sync_all()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
