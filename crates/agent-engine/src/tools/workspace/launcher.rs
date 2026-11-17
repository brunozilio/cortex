use std::process::Command;
use std::sync::Arc;

use platform::sandbox::Confinement;

pub trait Launcher: Send + Sync {
    fn launch(&self, command: Command, confinement: Option<&Confinement>) -> Command;

    fn support(&self) -> platform::sandbox::Support {
        platform::sandbox::support()
    }
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

    struct Unboxed;

    impl Launcher for Unboxed {
        fn launch(&self, command: Command, _confinement: Option<&Confinement>) -> Command {
            command
        }

        fn support(&self) -> platform::sandbox::Support {
            platform::sandbox::Support::Missing("this system has no sandbox".into())
        }
    }

    #[test]
    fn a_sandbox_the_system_cannot_provide_is_said_and_refuses_only_when_required() {
        let mut workspace = Workspace::new(std::env::temp_dir());
        workspace.services.launch = Launch(Arc::new(Unboxed));
        assert!(workspace.sandbox_missing().is_none() && workspace.sandbox_refusal().is_none(), "sandbox off");
        workspace.policy.sandbox = Some(domain::sandbox::Sandbox { required: false, ..Default::default() });
        assert!(!workspace.sandboxed(), "never claims to confine what it cannot");
        let missing = workspace.sandbox_missing().expect("said");
        assert!(!missing.refuses && missing.notice.contains("this system has no sandbox"), "{missing:?}");
        assert_eq!(workspace.sandbox_refusal(), None);
        workspace.policy.sandbox = Some(domain::sandbox::Sandbox { required: true, ..Default::default() });
        assert_eq!(workspace.sandbox_refusal().as_deref(), Some("this system has no sandbox"));
        assert!(workspace.sandbox_missing().expect("said").refuses);
    }

    #[test]
    fn by_default_a_command_runs_on_this_machine_as_given() {
        let launched = Workspace::new(std::env::temp_dir()).confined(Command::new("ls"));
        assert_eq!(launched.get_program(), "ls");
    }
}
