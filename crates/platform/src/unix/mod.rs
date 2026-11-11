mod private;
mod tree;

use std::fs::OpenOptions;
use std::path::Path;
use std::process::Command;

pub use private::{keep_owner, restrict, restrict_open};
pub use tree::Tree;

pub fn command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    Command::new(program)
}

pub fn shell(code: &str) -> Command {
    let program = std::env::var("SHELL").ok().filter(|shell| !shell.is_empty()).unwrap_or_else(|| "/bin/sh".into());
    let mut shell = command(program);
    shell.arg("-c").arg(code);
    shell
}

pub fn script_shell(code: &str) -> Command {
    let mut shell = command("/bin/sh");
    shell.args(["-c", code]);
    shell
}

pub fn shell_name() -> String {
    "the user's shell".to_owned()
}

pub fn make_executable(file: &std::fs::File) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    file.set_permissions(std::fs::Permissions::from_mode(0o755))
}

pub fn make_private_dir(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
}

pub fn nonblocking(options: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;
    options.custom_flags(libc::O_NONBLOCK);
}

pub fn capture(program: &str, args: &[&str]) -> Option<Vec<u8>> {
    let output = Command::new(program).args(args).output().ok()?;
    (output.status.success() && !output.stdout.is_empty()).then_some(output.stdout)
}
