pub mod bounded;
pub mod private_file;

#[cfg(unix)]
mod unix;

#[cfg(target_os = "linux")]
#[path = "linux/mod.rs"]
mod system;

#[cfg(target_os = "macos")]
#[path = "macos/mod.rs"]
mod system;

#[cfg(windows)]
#[path = "windows/mod.rs"]
mod system;

pub use bounded::{Bounds, Ended, Finished};
pub use system::*;

pub fn data_dir(name: &str, command: &str) -> Option<std::path::PathBuf> {
    system::data_dir_in(|key| std::env::var_os(key), std::env::home_dir(), name, command)
}

pub fn local_data_dir(name: &str, command: &str) -> Option<std::path::PathBuf> {
    system::local_data_dir_in(|key| std::env::var_os(key), std::env::home_dir(), name, command)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_command_is_the_program_asked_for() {
        let command = command("git");
        assert_eq!(command.get_program(), "git");
        assert!(!NAME.is_empty());
    }
}
