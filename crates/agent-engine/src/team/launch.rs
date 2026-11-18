use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use domain::PermissionMode;
use domain::team::{LEAD, TeammateMode, file_name};

use super::{AGENT_VARIABLE, TEAM_VARIABLE, Team};

pub const COMMAND_VARIABLE: &str = concat!(brand::env_prefix!(), "_TEAMMATE_COMMAND");
pub const SUBCOMMAND: &str = "teammate";
const TMUX: &str = "tmux";

#[derive(Debug, Clone)]
pub struct Launching<'a> {
    pub team: &'a Team,
    pub name: &'a str,
    pub cwd: &'a Path,
    pub prompt: &'a str,
    pub instructions: &'a str,
    pub model: &'a str,
    pub mode: PermissionMode,
    pub placed: TeammateMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Planned {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub env: Vec<(String, String)>,
    pub log: Option<PathBuf>,
}

fn teammate_args(launching: &Launching<'_>) -> Vec<OsString> {
    vec![
        "-C".into(),
        launching.cwd.as_os_str().to_owned(),
        "--model".into(),
        launching.model.into(),
        "--mode".into(),
        domain::command::mode_label(launching.mode).into(),
        "--append-system-prompt".into(),
        launching.instructions.into(),
        SUBCOMMAND.into(),
    ]
}

pub fn planned(launching: &Launching<'_>, program: PathBuf) -> Planned {
    let env = vec![
        (TEAM_VARIABLE.to_owned(), launching.team.name.clone()),
        (AGENT_VARIABLE.to_owned(), launching.name.to_owned()),
    ];
    if launching.placed == TeammateMode::Tmux {
        let mut args: Vec<OsString> =
            vec!["split-window".into(), "-d".into(), "-h".into(), "-c".into(), launching.cwd.as_os_str().to_owned()];
        for (key, value) in &env {
            args.push("-e".into());
            args.push(format!("{key}={value}").into());
        }
        args.push(program.into_os_string());
        args.extend(teammate_args(launching));
        return Planned { program: PathBuf::from(TMUX), args, env: Vec::new(), log: None };
    }
    Planned {
        program,
        args: teammate_args(launching),
        env,
        log: Some(launching.team.dir.join("logs").join(format!("{}.log", file_name(launching.name)))),
    }
}

fn program() -> Result<PathBuf, String> {
    match std::env::var_os(COMMAND_VARIABLE).filter(|command| !command.is_empty()) {
        Some(command) => Ok(PathBuf::from(command)),
        None => std::env::current_exe().map_err(|error| format!("the app's own program cannot be found: {error}")),
    }
}

pub fn inside_tmux() -> bool {
    std::env::var_os("TMUX").is_some_and(|session| !session.is_empty())
}

pub fn launch(launching: &Launching<'_>) -> Result<(), String> {
    launching.team.send(launching.name, LEAD, launching.prompt, None)?;
    let planned = planned(launching, program()?);
    let mut command = Command::new(&planned.program);
    command
        .args(&planned.args)
        .envs(planned.env.iter().map(|(key, value)| (key, value)))
        .current_dir(launching.cwd)
        .stdin(Stdio::null());
    let Some(log) = &planned.log else {
        let placed = command.output().map_err(|error| format!("tmux could not be started: {error}"))?;
        if !placed.status.success() {
            return Err(format!(
                "tmux could not open a pane for the teammate: {}",
                String::from_utf8_lossy(&placed.stderr).trim()
            ));
        }
        return Ok(());
    };
    if let Some(dir) = log.parent() {
        std::fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    }
    let out = std::fs::File::create(log).map_err(|error| error.to_string())?;
    let errors = out.try_clone().map_err(|error| error.to_string())?;
    let mut child =
        command.stdout(out).stderr(errors).spawn().map_err(|error| format!("the teammate could not start: {error}"))?;
    let team = launching.team.clone();
    let name = launching.name.to_owned();
    std::thread::Builder::new()
        .name(format!("teammate-{name}"))
        .spawn(move || {
            let _ = child.wait();
            team.departed(&name);
        })
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn launching(team: &Team, placed: TeammateMode) -> Launching<'_> {
        Launching {
            team,
            name: "tester",
            cwd: Path::new("/work"),
            prompt: "fix the tests",
            instructions: "# Team coordination",
            model: "claude-sonnet-5-5",
            mode: PermissionMode::AcceptEdits,
            placed,
        }
    }

    fn strings(args: &[OsString]) -> Vec<String> {
        args.iter().map(|arg| arg.to_string_lossy().into_owned()).collect()
    }

    #[test]
    fn a_teammate_runs_the_app_s_own_program_in_its_team_with_its_name_mode_and_instructions() {
        let team = Team::named(Path::new("/home/me"), "team-1");

        let planned = planned(&launching(&team, TeammateMode::Process), PathBuf::from("/bin/app"));

        let args = strings(&planned.args);
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
        assert!(planned.log.is_some_and(|log| log.ends_with("teams/team-1/logs/tester.log")));
        assert!(!args.iter().any(|arg| arg.contains("fix the tests")), "the task goes through the mailbox");
    }

    #[test]
    fn in_tmux_a_teammate_opens_in_a_pane_beside_the_lead_with_no_shell_in_between() {
        let team = Team::named(Path::new("/home/me"), "team-1");

        let planned = planned(&launching(&team, TeammateMode::Tmux), PathBuf::from("/bin/app"));

        assert_eq!(planned.program, PathBuf::from("tmux"));
        let args = strings(&planned.args);
        assert_eq!(args[..5], ["split-window", "-d", "-h", "-c", "/work"]);
        assert!(
            args.contains(&format!("{TEAM_VARIABLE}=team-1")) && args.contains(&format!("{AGENT_VARIABLE}=tester"))
        );
        let at = args.iter().position(|arg| arg == "/bin/app").expect("the program");
        assert_eq!(args[at + 1..].last().map(String::as_str), Some("teammate"));
        assert_eq!(args[at + 1..].len(), 9, "each of the app's arguments stays its own argument");
        assert_eq!(planned.log, None, "the pane shows what it prints");
    }
}
