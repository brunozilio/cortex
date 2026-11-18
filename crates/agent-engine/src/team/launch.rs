use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use domain::PermissionMode;
use domain::team::{LEAD, file_name};

use super::{AGENT_VARIABLE, TEAM_VARIABLE, Team};

pub const COMMAND_VARIABLE: &str = concat!(brand::env_prefix!(), "_TEAMMATE_COMMAND");
pub const SUBCOMMAND: &str = "teammate";

#[derive(Debug, Clone)]
pub struct Launching<'a> {
    pub team: &'a Team,
    pub name: &'a str,
    pub cwd: &'a Path,
    pub prompt: &'a str,
    pub instructions: &'a str,
    pub model: &'a str,
    pub mode: PermissionMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Planned {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub env: Vec<(String, String)>,
    pub log: PathBuf,
}

pub fn planned(launching: &Launching<'_>, program: PathBuf) -> Planned {
    let mode = domain::command::mode_label(launching.mode);
    let args: Vec<OsString> = vec![
        "-C".into(),
        launching.cwd.as_os_str().to_owned(),
        "--model".into(),
        launching.model.into(),
        "--mode".into(),
        mode.into(),
        "--append-system-prompt".into(),
        launching.instructions.into(),
        SUBCOMMAND.into(),
    ];
    Planned {
        program,
        args,
        env: vec![
            (TEAM_VARIABLE.to_owned(), launching.team.name.clone()),
            (AGENT_VARIABLE.to_owned(), launching.name.to_owned()),
        ],
        log: launching.team.dir.join("logs").join(format!("{}.log", file_name(launching.name))),
    }
}

fn program() -> Result<PathBuf, String> {
    match std::env::var_os(COMMAND_VARIABLE).filter(|command| !command.is_empty()) {
        Some(command) => Ok(PathBuf::from(command)),
        None => std::env::current_exe().map_err(|error| format!("the app's own program cannot be found: {error}")),
    }
}

pub fn launch(launching: &Launching<'_>) -> Result<(), String> {
    launching.team.send(launching.name, LEAD, launching.prompt, None)?;
    let planned = planned(launching, program()?);
    if let Some(dir) = planned.log.parent() {
        std::fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    }
    let log = std::fs::File::create(&planned.log).map_err(|error| error.to_string())?;
    let errors = log.try_clone().map_err(|error| error.to_string())?;
    let mut child = Command::new(&planned.program)
        .args(&planned.args)
        .envs(planned.env.iter().map(|(key, value)| (key, value)))
        .current_dir(launching.cwd)
        .stdin(Stdio::null())
        .stdout(log)
        .stderr(errors)
        .spawn()
        .map_err(|error| format!("the teammate could not start: {error}"))?;
    let team = launching.team.clone();
    let name = launching.name.to_owned();
    std::thread::Builder::new()
        .name(format!("teammate-{name}"))
        .spawn(move || {
            let _ = child.wait();
            team.left(&name);
        })
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_teammate_runs_the_app_s_own_program_in_its_team_with_its_name_mode_and_instructions() {
        let team = Team::named(Path::new("/home/me"), "team-1");
        let launching = Launching {
            team: &team,
            name: "tester",
            cwd: Path::new("/work"),
            prompt: "fix the tests",
            instructions: "# Team coordination",
            model: "claude-sonnet-5-5",
            mode: PermissionMode::AcceptEdits,
        };

        let planned = planned(&launching, PathBuf::from("/bin/app"));

        let args: Vec<String> = planned.args.iter().map(|arg| arg.to_string_lossy().into_owned()).collect();
        assert_eq!(
            args,
            [
                "-C",
                "/work",
                "--model",
                "claude-sonnet-5-5",
                "--mode",
                domain::command::mode_label(PermissionMode::AcceptEdits),
                "--append-system-prompt",
                "# Team coordination",
                "teammate"
            ]
        );
        assert!(planned.env.contains(&(TEAM_VARIABLE.to_owned(), "team-1".to_owned())));
        assert!(planned.env.contains(&(AGENT_VARIABLE.to_owned(), "tester".to_owned())));
        assert!(planned.log.ends_with("teams/team-1/logs/tester.log"), "{}", planned.log.display());
        assert!(!args.iter().any(|arg| arg.contains("fix the tests")), "the task goes through the mailbox");
    }
}
