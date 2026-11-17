#![allow(unsafe_code)]

use std::io;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;

use landlock::{
    ABI, AccessFs, AccessNet, CompatLevel, Compatible, PathBeneath, PathFd, Ruleset, RulesetAttr, RulesetCreated,
    RulesetCreatedAttr, RulesetError, RulesetStatus,
};

use crate::sandbox::{Confinement, Support};

const NEWEST: ABI = ABI::V5;
const DEVICES: [&str; 5] = ["/dev/null", "/dev/zero", "/dev/tty", "/dev/ptmx", "/dev/full"];

pub fn support() -> Support {
    static FOUND: OnceLock<Support> = OnceLock::new();
    FOUND.get_or_init(probe).clone()
}

fn probe() -> Support {
    let made = Ruleset::default()
        .set_compatibility(CompatLevel::HardRequirement)
        .handle_access(AccessFs::from_write(ABI::V1))
        .and_then(Ruleset::create);
    match made {
        Ok(_) => Support::Available,
        Err(error) => Support::Missing(format!(
            "this Linux kernel does not provide Landlock (Linux 5.13 or later, with Landlock enabled at boot): {error}"
        )),
    }
}

fn ruleset(confinement: &Confinement) -> Result<RulesetCreated, RulesetError> {
    let writes = AccessFs::from_write(NEWEST);
    let mut ruleset = Ruleset::default().set_compatibility(CompatLevel::BestEffort).handle_access(writes)?;
    if !confinement.network {
        ruleset = ruleset.handle_access(AccessNet::ConnectTcp)?;
    }
    let mut created = ruleset.create()?;
    for path in confinement.writable.iter().map(std::path::PathBuf::as_path) {
        if let Some(rule) = beneath(path, writes) {
            created = created.add_rule(rule)?;
        }
    }
    for device in DEVICES.map(Path::new) {
        if let Some(rule) = beneath(device, AccessFs::WriteFile | AccessFs::Truncate) {
            created = created.add_rule(rule)?;
        }
    }
    Ok(created)
}

fn beneath(path: &Path, access: landlock::BitFlags<AccessFs>) -> Option<PathBeneath<PathFd>> {
    let opened = PathFd::new(path).ok()?;
    let access = if path.is_dir() { access } else { access & AccessFs::from_file(NEWEST) };
    Some(PathBeneath::new(opened, access))
}

fn restrict(ruleset: Option<RulesetCreated>) -> io::Result<()> {
    let refused = || io::Error::from(io::ErrorKind::PermissionDenied);
    let ruleset = ruleset.ok_or_else(refused)?;
    match ruleset.restrict_self() {
        Ok(status) if status.ruleset != RulesetStatus::NotEnforced => Ok(()),
        _ => Err(refused()),
    }
}

pub fn confined(mut command: Command, confinement: &Confinement) -> Command {
    if support() != Support::Available {
        return command;
    }
    command.envs(confinement.environment.iter().map(|(key, value)| (key, value)));
    let mut ruleset = match ruleset(confinement) {
        Ok(ruleset) => Some(ruleset),
        Err(error) => {
            tracing::error!(%error, "the sandbox could not be prepared; the command is refused");
            None
        }
    };
    unsafe {
        command.pre_exec(move || restrict(ruleset.take()));
    }
    command
}

#[cfg(test)]
mod tests {
    use std::process::Stdio;

    use super::*;

    fn runs(code: &str, dir: &Path, confinement: &Confinement) -> bool {
        let mut shell = Command::new("/bin/sh");
        shell.args(["-c", code]).current_dir(dir);
        let mut confined = confined(shell, confinement);
        confined.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        confined.status().is_ok_and(|status| status.success())
    }

    #[test]
    fn a_confined_command_writes_inside_its_folders_and_nowhere_else() {
        if support() != Support::Available {
            return;
        }
        let project = tempfile::tempdir().expect("project");
        let elsewhere = tempfile::tempdir().expect("elsewhere");
        let confinement =
            Confinement { writable: vec![project.path().to_path_buf()], network: true, ..Confinement::default() };
        assert!(runs("echo in > made.txt", project.path(), &confinement));
        assert!(project.path().join("made.txt").exists());
        let outside = elsewhere.path().join("made.txt");
        assert!(!runs(&format!("echo out > '{}'", outside.display()), project.path(), &confinement));
        assert!(!outside.exists());
        assert!(runs("echo quiet > /dev/null", project.path(), &confinement), "devices stay writable");
        assert!(runs(&format!("cat '{}/made.txt'", project.path().display()), elsewhere.path(), &confinement));
    }

    #[test]
    fn the_answer_about_landlock_is_the_same_each_time() {
        assert_eq!(support(), support());
        if let Support::Missing(why) = support() {
            assert!(why.contains("Landlock"), "{why}");
        }
    }
}
