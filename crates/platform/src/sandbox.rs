use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Confinement {
    pub writable: Vec<PathBuf>,
    pub protected: Vec<PathBuf>,
    pub network: bool,
    pub environment: Vec<(String, String)>,
    pub sockets: Vec<PathBuf>,
    pub unreadable: Vec<PathBuf>,
    pub readable: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Support {
    Available,
    Missing(String),
}

pub fn support() -> Support {
    #[cfg(target_os = "macos")]
    {
        Support::Available
    }
    #[cfg(target_os = "linux")]
    {
        crate::system::landlock::support()
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        Support::Missing(format!("{} has no sandbox for commands", crate::NAME))
    }
}

pub fn supported() -> bool {
    support() == Support::Available
}

pub fn confined(command: Command, confinement: &Confinement) -> Command {
    #[cfg(target_os = "macos")]
    {
        crate::system::seatbelt::confined(&command, confinement)
    }
    #[cfg(target_os = "linux")]
    {
        crate::system::landlock::confined(command, confinement)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = confinement;
        command
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whether_commands_can_be_confined_is_answered_with_a_reason_when_they_cannot() {
        match support() {
            Support::Available => assert!(supported()),
            Support::Missing(why) => {
                assert!(!supported());
                assert!(!why.is_empty());
            }
        }
        if cfg!(target_os = "macos") {
            assert!(supported(), "macOS always has sandbox-exec");
        }
    }

    #[test]
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    fn a_system_without_a_sandbox_says_so() {
        let mut command = Command::new("git");
        command.arg("status");
        let confined = confined(command, &Confinement::default());
        assert_eq!(confined.get_program(), "git");
        assert!(matches!(support(), Support::Missing(why) if why.contains("no sandbox")));
    }
}
