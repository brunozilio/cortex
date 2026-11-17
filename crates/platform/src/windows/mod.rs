mod private;
mod shell;
mod tree;

use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::Command;

pub use shell::WindowsShell;
pub use tree::alive;
pub(crate) use tree::{Tree, lost};

pub const NAME: &str = "Windows";
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn release_target() -> Option<&'static str> {
    None
}

pub fn port_holder(_port: u16) -> Option<(u32, String)> {
    None
}

pub fn terminate(_pid: u32) -> std::io::Result<()> {
    Err(std::io::Error::from(std::io::ErrorKind::Unsupported))
}

pub fn command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    use std::os::windows::process::CommandExt;
    let mut command = Command::new(program);
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

pub fn shell(code: &str) -> Command {
    WindowsShell::detect().command(code)
}

pub fn script_shell(code: &str) -> Command {
    WindowsShell::detect().command(code)
}

pub fn shell_name() -> String {
    WindowsShell::detect().name().to_owned()
}

pub fn restrict_open(_options: &mut OpenOptions) {}

pub fn restrict(_file: &File, path: &Path) -> std::io::Result<()> {
    private::restrict(path)
}

pub fn keep_owner(_file: &File, _path: &Path) -> std::io::Result<()> {
    Ok(())
}

pub fn open_with_mode_of(options: &mut OpenOptions, path: &Path, _existing: &Path) -> std::io::Result<File> {
    options.open(path)
}

pub fn is_file_at(_file: &File, _path: &Path) -> bool {
    true
}

pub const LOCK_FILES_CAN_BE_REMOVED: bool = false;

pub fn make_executable(_file: &File) -> std::io::Result<()> {
    Ok(())
}

pub fn make_private_dir(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

pub fn nonblocking(_options: &mut OpenOptions) {}

pub fn open_url(url: &str) -> Command {
    let mut open = command("explorer");
    open.arg(url);
    open
}

pub fn notification(title: &str, message: &str) -> (String, Vec<String>, Vec<(String, String)>) {
    let script = "[reflection.assembly]::loadwithpartialname('System.Windows.Forms') | Out-Null; \
         $n = New-Object System.Windows.Forms.NotifyIcon; $n.Icon = [System.Drawing.SystemIcons]::Information; \
         $n.Visible = $true; $n.ShowBalloonTip(5000, $env:APP_NOTIFICATION_TITLE, $env:APP_NOTIFICATION_MESSAGE, 'Info'); \
         Start-Sleep 6";
    let env =
        vec![("APP_NOTIFICATION_TITLE".into(), title.into()), ("APP_NOTIFICATION_MESSAGE".into(), message.into())];
    ("powershell".into(), vec!["-NoProfile".into(), "-Command".into(), script.into()], env)
}

pub fn clipboard_image() -> Option<Vec<u8>> {
    None
}

pub fn data_dir_in(
    var: impl Fn(&str) -> Option<OsString>,
    home: Option<PathBuf>,
    name: &str,
    _command: &str,
) -> Option<PathBuf> {
    var("APPDATA")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| home.map(|home| home.join("AppData\\Roaming")))
        .map(|dir| dir.join(name))
}

pub fn local_data_dir_in(
    var: impl Fn(&str) -> Option<OsString>,
    home: Option<PathBuf>,
    name: &str,
    _command: &str,
) -> Option<PathBuf> {
    var("LOCALAPPDATA")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| home.map(|home| home.join("AppData\\Local")))
        .map(|dir| dir.join(name))
}
