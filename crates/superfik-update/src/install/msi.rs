//! Windows' per-machine MSI install (`%ProgramFiles%\Superfik`). The
//! verified MSI is kept in a staging folder after its `ProductVersion` is
//! checked; once the app has quit, `swap.ps1` runs it through `msiexec`
//! (Windows asks for consent to elevate; the MSI's major upgrade replaces
//! the old install) and relaunches the app, the old one if consent was
//! refused.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::{Commands, Installer, Prepared, write_script};
use crate::{UpdateError, Version};

pub const MSI_SWAP_SCRIPT: &str = include_str!("swap.ps1");

const POWERSHELL: &str = "powershell.exe";

pub struct Msi {
    exe: PathBuf,
    staging: PathBuf,
    commands: Box<dyn Commands>,
    pid: u32,
    scripts: PathBuf,
}

impl std::fmt::Debug for Msi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Msi").field("exe", &self.exe).field("staging", &self.staging).finish()
    }
}

impl Msi {
    pub fn new(exe: PathBuf, staging: PathBuf, commands: Box<dyn Commands>, pid: u32, scripts: PathBuf) -> Self {
        Self { exe, staging, commands, pid, scripts }
    }

    fn product_version(&self, msi: &Path) -> Result<String, UpdateError> {
        let args: Vec<OsString> =
            vec!["-NoProfile".into(), "-NonInteractive".into(), "-Command".into(), version_query(msi).into()];
        let ran = self.commands.run(POWERSHELL, &args)?;
        if !ran.success {
            return Err(UpdateError::Install(format!("reading the MSI: {}", ran.output.trim())));
        }
        Ok(ran.output.trim().to_owned())
    }
}

impl Installer for Msi {
    fn stage(&self, archive: &Path, version: &Version) -> Result<Prepared, UpdateError> {
        if self.staging.exists() {
            std::fs::remove_dir_all(&self.staging).map_err(UpdateError::install)?;
        }
        std::fs::create_dir_all(&self.staging).map_err(UpdateError::install)?;
        let staged = self.staging.join(format!("Superfik-{version}-x64.msi"));
        let checked = std::fs::copy(archive, &staged).map_err(UpdateError::install).and_then(|_| {
            match self.product_version(&staged)? {
                found if found == version.to_string() => Ok(()),
                found => Err(UpdateError::Install(format!("the update is {found}, not {version}"))),
            }
        });
        match checked {
            Ok(()) => Ok(Prepared { version: *version, staged }),
            Err(error) => {
                let _ = std::fs::remove_dir_all(&self.staging);
                Err(error)
            }
        }
    }

    fn restart(&self, prepared: &Prepared) -> Result<(), UpdateError> {
        if prepared.staged.parent() != Some(self.staging.as_path()) || !prepared.staged.is_file() {
            return Err(UpdateError::Install("nothing is staged".into()));
        }
        let script = write_script(&self.scripts, "swap.ps1", MSI_SWAP_SCRIPT)?;
        let args: Vec<OsString> = vec![
            "-NoProfile".into(),
            "-NonInteractive".into(),
            "-ExecutionPolicy".into(),
            "Bypass".into(),
            "-WindowStyle".into(),
            "Hidden".into(),
            "-File".into(),
            script.into(),
            self.pid.to_string().into(),
            prepared.staged.clone().into(),
            self.exe.clone().into(),
        ];
        self.commands.detach(POWERSHELL, &args)
    }
}

pub fn version_query(msi: &Path) -> String {
    let path = msi.to_string_lossy().replace('\'', "''");
    format!(
        "$ErrorActionPreference = 'Stop'; \
         $i = New-Object -ComObject WindowsInstaller.Installer; \
         $d = $i.GetType().InvokeMember('OpenDatabase', 'InvokeMethod', $null, $i, @('{path}', 0)); \
         $v = $d.GetType().InvokeMember('OpenView', 'InvokeMethod', $null, $d, @(\"SELECT Value FROM Property WHERE Property='ProductVersion'\")); \
         [void]$v.GetType().InvokeMember('Execute', 'InvokeMethod', $null, $v, $null); \
         $r = $v.GetType().InvokeMember('Fetch', 'InvokeMethod', $null, $v, $null); \
         $r.GetType().InvokeMember('StringData', 'GetProperty', $null, $r, 1); \
         [void]$v.GetType().InvokeMember('Close', 'InvokeMethod', $null, $v, $null)"
    )
}

pub fn msi_install_of(exe: &Path) -> Option<PathBuf> {
    let named = exe.file_name()?.to_string_lossy().eq_ignore_ascii_case("superfik.exe");
    let folder = exe.parent()?.file_name()?.to_string_lossy().eq_ignore_ascii_case("Superfik");
    (named && folder).then(|| exe.to_path_buf())
}
