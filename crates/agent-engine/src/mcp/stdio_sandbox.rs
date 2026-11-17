use domain::sandbox::{Places, Sandbox, ServerSandbox};
use platform::sandbox::{Confinement, Support};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StdioSandbox {
    Declared(ServerSandbox),
    Unconfined,
    Confined(Confinement),
    Refused(String),
}

impl Default for StdioSandbox {
    fn default() -> Self {
        Self::Declared(ServerSandbox::default())
    }
}

#[derive(Debug)]
pub struct Settled<'a> {
    pub sandbox: Option<&'a Sandbox>,
    pub places: &'a Places<'a>,
    pub support: &'a Support,
}

impl StdioSandbox {
    #[must_use]
    pub fn settled(self, owner: bool, settled: &Settled<'_>) -> Self {
        let Self::Declared(declared) = self else { return self };
        let declared = declared.as_declared_by(owner);
        let Some(sandbox) = settled.sandbox else { return Self::Unconfined };
        if declared.off {
            return Self::Unconfined;
        }
        match settled.support {
            Support::Missing(why) if sandbox.required => Self::Refused(domain::sandbox::refusal(why)),
            Support::Missing(_) => Self::Unconfined,
            Support::Available => {
                let found = sandbox.server_confinement(settled.places, &declared);
                Self::Confined(crate::tools::from_domain(found, settled.places))
            }
        }
    }

    pub fn applied(&self, name: &str, command: std::process::Command) -> Result<std::process::Command, String> {
        match self {
            Self::Confined(confinement) => Ok(platform::sandbox::confined(command, confinement)),
            Self::Refused(why) => Err(format!("{name} could not start: {why}")),
            Self::Declared(_) | Self::Unconfined => Ok(command),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;

    fn places<'a>(temp: &'a [PathBuf]) -> Places<'a> {
        Places {
            project: Path::new("/repo"),
            added: &[],
            repository: None,
            temp,
            home: Some(Path::new("/Users/ana")),
            settings_dir: ".app",
            data: &[],
        }
    }

    fn settle(declared: ServerSandbox, owner: bool, sandbox: Option<&Sandbox>, support: &Support) -> StdioSandbox {
        let temp = [PathBuf::from("/tmp")];
        let places = places(&temp);
        StdioSandbox::Declared(declared).settled(owner, &Settled { sandbox, places: &places, support })
    }

    #[test]
    fn a_stdio_server_runs_in_the_sandbox_when_it_is_on() {
        let sandbox = Sandbox::default();
        let settled = settle(ServerSandbox::default(), false, Some(&sandbox), &Support::Available);
        let StdioSandbox::Confined(confinement) = settled else { panic!("{settled:?}") };
        assert!(confinement.writable.contains(&PathBuf::from("/repo")));
        assert!(confinement.writable.contains(&PathBuf::from("/tmp")));
        assert!(confinement.network);
        assert_eq!(settle(ServerSandbox::default(), true, None, &Support::Available), StdioSandbox::Unconfined);
    }

    #[test]
    fn only_the_owner_s_entry_turns_a_server_s_sandbox_off() {
        let sandbox = Sandbox::default();
        let off = ServerSandbox { off: true, ..ServerSandbox::default() };
        assert_eq!(settle(off.clone(), true, Some(&sandbox), &Support::Available), StdioSandbox::Unconfined);
        assert!(matches!(settle(off, false, Some(&sandbox), &Support::Available), StdioSandbox::Confined(_)));
    }

    #[test]
    fn a_required_sandbox_the_system_lacks_refuses_the_server_instead_of_running_it_unconfined() {
        let missing = Support::Missing("this kernel has no Landlock".into());
        let required = Sandbox { required: true, ..Sandbox::default() };
        let refused = settle(ServerSandbox::default(), true, Some(&required), &missing);
        assert!(matches!(&refused, StdioSandbox::Refused(why) if why.contains("sandbox.required")), "{refused:?}");
        let error = refused.applied("jira", std::process::Command::new("true")).expect_err("refused");
        assert!(error.starts_with("jira could not start") && error.contains("no Landlock"), "{error}");
        let optional = Sandbox { required: false, ..Sandbox::default() };
        assert_eq!(settle(ServerSandbox::default(), true, Some(&optional), &missing), StdioSandbox::Unconfined);
    }
}
