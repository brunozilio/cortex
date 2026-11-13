use std::path::PathBuf;
use std::process::Command;

pub const SUPPORTED: bool = cfg!(target_os = "macos");

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Confinement {
    pub writable: Vec<PathBuf>,
    pub protected: Vec<PathBuf>,
    pub network: bool,
}

pub fn confined(command: Command, confinement: &Confinement) -> Command {
    #[cfg(target_os = "macos")]
    {
        crate::system::seatbelt::confined(&command, confinement)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = confinement;
        command
    }
}

#[cfg(test)]
#[cfg(not(target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn a_system_without_a_sandbox_runs_the_command_as_it_is() {
        let mut command = Command::new("git");
        command.arg("status");
        let confined = confined(command, &Confinement::default());
        assert_eq!(confined.get_program(), "git");
        assert!(!SUPPORTED);
    }
}
