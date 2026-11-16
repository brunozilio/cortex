use std::process::Command;
use std::sync::Arc;

use platform::sandbox::Confinement;

pub trait Launcher: Send + Sync {
    fn launch(&self, command: Command, confinement: Option<&Confinement>) -> Command;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct OnThisMachine;

impl Launcher for OnThisMachine {
    fn launch(&self, command: Command, confinement: Option<&Confinement>) -> Command {
        match confinement {
            Some(confinement) => platform::sandbox::confined(command, confinement),
            None => command,
        }
    }
}

#[derive(Clone)]
pub struct Launch(pub Arc<dyn Launcher>);

impl Default for Launch {
    fn default() -> Self {
        Self(Arc::new(OnThisMachine))
    }
}

impl std::fmt::Debug for Launch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Launch")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::Workspace;

    struct InABox;

    impl Launcher for InABox {
        fn launch(&self, command: Command, _confinement: Option<&Confinement>) -> Command {
            let mut boxed = Command::new("box");
            boxed.arg(command.get_program());
            boxed.args(command.get_args());
            boxed
        }
    }

    #[test]
    fn a_workspace_starts_its_commands_through_the_launcher_it_was_given() {
        let mut workspace = Workspace::new(std::env::temp_dir());
        workspace.services.launch = Launch(Arc::new(InABox));
        let mut command = Command::new("ls");
        command.arg("-l");
        let launched = workspace.confined(command);
        assert_eq!(launched.get_program(), "box");
        assert_eq!(launched.get_args().collect::<Vec<_>>(), ["ls", "-l"]);
    }

    #[test]
    fn by_default_a_command_runs_on_this_machine_as_given() {
        let launched = Workspace::new(std::env::temp_dir()).confined(Command::new("ls"));
        assert_eq!(launched.get_program(), "ls");
    }
}
