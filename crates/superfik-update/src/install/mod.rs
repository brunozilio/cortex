//! Installing a verified archive over the running install: unpacked and
//! checked in a staging folder beside it, then swapped into place by a
//! detached script once the app has quit (`swap.sh`), which relaunches it.
//! Windows' MSI install is replaced by the MSI itself (`msi`).

mod layouts;
mod msi;
mod system;

use std::ffi::OsString;
use std::path::{Path, PathBuf};

pub use layouts::{MacApp, TEAM_ID, Tarball, app_bundle_of, short_version, tarball_root_of, team_of};
pub use msi::{MSI_SWAP_SCRIPT, Msi, msi_install_of, version_query};
pub use system::{Commands, Ran, System};

use crate::{UpdateError, Version};

pub const SWAP_SCRIPT: &str = include_str!("swap.sh");

const STAGING: &str = ".superfik-update";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prepared {
    pub version: Version,
    pub staged: PathBuf,
}

pub trait Installer: Send + Sync {
    fn stage(&self, archive: &Path, version: &Version) -> Result<Prepared, UpdateError>;
    fn restart(&self, prepared: &Prepared) -> Result<(), UpdateError>;
}

pub trait Layout: Send + Sync {
    fn unpack(&self, archive: &Path, into: &Path) -> Result<(), UpdateError>;
    fn check(&self, unpacked: &Path, version: &Version) -> Result<(), UpdateError>;
    fn relaunch(&self, current: &Path) -> Vec<OsString>;
}

pub struct SwapInstaller {
    current: PathBuf,
    layout: Box<dyn Layout>,
    commands: Box<dyn Commands>,
    pid: u32,
    scripts: PathBuf,
}

impl std::fmt::Debug for SwapInstaller {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SwapInstaller").field("current", &self.current).field("pid", &self.pid).finish()
    }
}

impl SwapInstaller {
    pub fn new(
        current: PathBuf,
        layout: Box<dyn Layout>,
        commands: Box<dyn Commands>,
        pid: u32,
        scripts: PathBuf,
    ) -> Self {
        Self { current, layout, commands, pid, scripts }
    }

    fn staging(&self) -> Result<PathBuf, UpdateError> {
        let parent = self.current.parent().ok_or(UpdateError::Unsupported)?;
        Ok(parent.join(STAGING))
    }
}

impl Installer for SwapInstaller {
    fn stage(&self, archive: &Path, version: &Version) -> Result<Prepared, UpdateError> {
        let staging = self.staging()?;
        if staging.exists() {
            std::fs::remove_dir_all(&staging).map_err(UpdateError::install)?;
        }
        std::fs::create_dir_all(&staging).map_err(UpdateError::install)?;
        let staged = self.layout.unpack(archive, &staging).and_then(|()| {
            let unpacked = only_folder(&staging)?;
            self.layout.check(&unpacked, version)?;
            Ok(unpacked)
        });
        match staged {
            Ok(staged) => Ok(Prepared { version: *version, staged }),
            Err(error) => {
                let _ = std::fs::remove_dir_all(&staging);
                Err(error)
            }
        }
    }

    fn restart(&self, prepared: &Prepared) -> Result<(), UpdateError> {
        if prepared.staged.parent() != Some(self.staging()?.as_path()) || !prepared.staged.is_dir() {
            return Err(UpdateError::Install("nothing is staged".into()));
        }
        let script = write_script(&self.scripts, "swap.sh", SWAP_SCRIPT)?;
        let mut args: Vec<OsString> = vec![
            script.into(),
            self.pid.to_string().into(),
            self.current.clone().into(),
            prepared.staged.clone().into(),
        ];
        args.extend(self.layout.relaunch(&self.current));
        self.commands.detach("/bin/sh", &args)
    }
}

fn write_script(scripts: &Path, name: &str, body: &str) -> Result<PathBuf, UpdateError> {
    use std::io::Write;
    std::fs::create_dir_all(scripts).map_err(UpdateError::install)?;
    let mut folder = tempfile::Builder::new();
    folder.prefix("superfik-swap-");
    #[cfg(unix)]
    folder.permissions(std::os::unix::fs::PermissionsExt::from_mode(0o700));
    let own = folder.tempdir_in(scripts).map_err(UpdateError::install)?;
    let script = own.path().join(name);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o700);
    let mut file = options.open(&script).map_err(UpdateError::install)?;
    file.write_all(body.as_bytes()).map_err(UpdateError::install)?;
    let _kept = own.keep();
    Ok(script)
}

fn only_folder(dir: &Path) -> Result<PathBuf, UpdateError> {
    let mut folders = std::fs::read_dir(dir)
        .map_err(UpdateError::install)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .filter(|path| path.file_name().is_some_and(|n| !n.to_string_lossy().starts_with('.') && n != "__MACOSX"));
    match (folders.next(), folders.next()) {
        (Some(folder), None) => Ok(folder),
        _ => Err(UpdateError::Install("the archive does not hold one install".into())),
    }
}

pub fn installer() -> Option<Box<dyn Installer>> {
    let exe = std::env::current_exe().ok()?;
    if cfg!(target_os = "windows") {
        let exe = msi_install_of(&exe)?;
        let data = std::env::var_os("LOCALAPPDATA").map_or_else(std::env::temp_dir, PathBuf::from);
        let staging = data.join("Superfik").join("updates").join("msi");
        let installer = Msi::new(exe, staging, Box::new(System), std::process::id(), std::env::temp_dir());
        return Some(Box::new(installer));
    }
    let exe = exe.canonicalize().ok()?;
    let (current, layout): (PathBuf, Box<dyn Layout>) = if cfg!(target_os = "macos") {
        (app_bundle_of(&exe)?, Box::new(MacApp::new(Box::new(System))))
    } else if cfg!(target_os = "linux") {
        (tarball_root_of(&exe)?, Box::new(Tarball::new(Box::new(System))))
    } else {
        return None;
    };
    if !writable(current.parent()?) || !writable(&current) {
        return None;
    }
    let installer = SwapInstaller::new(current, layout, Box::new(System), std::process::id(), std::env::temp_dir());
    Some(Box::new(installer))
}

pub fn writable(dir: &Path) -> bool {
    let probe = dir.join(format!(".superfik-write-check-{}", std::process::id()));
    let made = std::fs::OpenOptions::new().write(true).create_new(true).open(&probe).is_ok();
    if made {
        let _ = std::fs::remove_file(&probe);
    }
    made
}
