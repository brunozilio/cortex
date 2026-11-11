mod github;

use std::io::{Read, Write};
use std::path::Path;

use domain::release::{self, Version};
use sha2::{Digest, Sha256};

pub use self::github::GitHub;

pub trait Releases {
    fn latest_tag(&self) -> Result<String, String>;
    fn download(&self, tag: &str, name: &str) -> Result<Vec<u8>, String>;
}

#[derive(Debug, Clone, Copy)]
pub struct Platform<'a> {
    pub arch: &'a str,
    pub os: &'a str,
}

impl Platform<'static> {
    pub fn this() -> Self {
        Self { arch: std::env::consts::ARCH, os: std::env::consts::OS }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Updated {
    AlreadyLatest,
    To(Version),
}

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
        }
    }
    Ok(())
}

pub fn update(releases: &dyn Releases, current: &str, exe: &Path, platform: Platform<'_>) -> Result<Updated, String> {
    let tag = releases.latest_tag()?;
    let Some(version) = release::newer(current, &tag) else {
        return Ok(Updated::AlreadyLatest);
    };
    let asset = release::asset(version, platform.arch, platform.os).ok_or_else(|| {
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
