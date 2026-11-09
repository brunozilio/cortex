//! Commands for the console tools Superfik starts (`git`, `gh`, `claude`,
//! `bw`…). Where a tool is found is `superfik_core::executable`; how it is
//! started is here, so the pure core never touches processes.

mod tree;
#[cfg(any(windows, test))]
mod windows_shell;

pub use tree::Tree;
#[cfg(windows)]
pub use windows_shell::WindowsShell;

pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn command(program: impl AsRef<std::ffi::OsStr>) -> std::process::Command {
    #[allow(unused_mut)]
    let mut command = std::process::Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_command_is_the_program_asked_for() {
        let command = command("git");
        assert_eq!(command.get_program(), "git");
        assert_eq!(command.get_args().count(), 0);
    }
}
