use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use domain::lsp::install::{Install, Recipe};
use platform::{Bounds, Ended};

use super::Starter;
use super::download::Download;
use super::installs::Installer;

const TIMEOUT: Duration = Duration::from_secs(10 * 60);
const OUTPUT: usize = 64 * 1024;

#[derive(Debug, Default)]
pub struct Downloads;

impl Installer for Downloads {
    fn install(&self, install: &Install, home: &Path) -> Result<(), String> {
        settle(home, &install.program, |staging| {
            fill(install, staging).and_then(|()| executable(&staging.join(&install.program)))
        })
    }
}

fn settle(home: &Path, program: &Path, fill: impl FnOnce(&Path) -> Result<(), String>) -> Result<(), String> {
    let parent = home.parent().ok_or("the install folder has no parent")?;
    let name = home.file_name().ok_or("the install folder has no name")?.to_string_lossy();
    let staging = parent.join(format!(".{name}.partial-{}", std::process::id()));
    let replaced = parent.join(format!(".{name}.replaced-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging).map_err(|error| format!("cannot create {}: {error}", staging.display()))?;
    let placed = fill(&staging).and_then(|()| place(&staging, home, program, &replaced));
    let _ = std::fs::remove_dir_all(&staging);
    placed
}

fn place(staging: &Path, home: &Path, program: &Path, replaced: &Path) -> Result<(), String> {
    let complete = || home.join(program).is_file();
    if complete() {
        return Ok(());
    }
    let _ = std::fs::remove_dir_all(replaced);
    let _ = std::fs::rename(home, replaced);
    let moved = std::fs::rename(staging, home);
    let _ = std::fs::remove_dir_all(replaced);
    match moved {
        Err(error) if !complete() => Err(format!("cannot move the install into place: {error}")),
        _ => Ok(()),
    }
}

fn fill(install: &Install, staging: &Path) -> Result<(), String> {
    match &install.recipe {
        Recipe::Npm { packages } => {
            let mut command = platform::command(tool(&install.recipe)?);
            command.arg("install").arg("--prefix").arg(staging).args(["--no-audit", "--no-fund", "--loglevel=error"]);
            run(command.args(packages))
        }
        Recipe::Go { module } => {
            let mut command = platform::command(tool(&install.recipe)?);
            run(command.arg("install").arg(module).env("GOBIN", staging.join("bin")))
        }
        Recipe::Download { url, archive, sha256 } => {
            Download { url, archive: *archive, sha256 }.unpack_into(staging, &staging.join(&install.program))
        }
    }
}

fn tool(recipe: &Recipe) -> Result<PathBuf, String> {
    let (name, runtime) = recipe.tool().ok_or("it runs no tool")?;
    super::Processes.find(name).ok_or_else(|| format!("it needs `{name}` on PATH; install {runtime} first."))
}

fn run(command: &mut Command) -> Result<(), String> {
    let never = AtomicBool::new(false);
    let finished =
        platform::bounded::run(command, Bounds { timeout: TIMEOUT, max_output: OUTPUT, stop: &never, input: None })
            .map_err(|error| error.to_string())?;
    match finished.ended {
        Ended::Exited(status) if status.success() => Ok(()),
        Ended::Exited(status) => Err(format!("{status}: {}", last_lines(&finished.stderr))),
        Ended::TimedOut => Err("it took longer than 10 minutes".into()),
        Ended::Stopped => Err("it was stopped".into()),
    }
}

fn last_lines(text: &str) -> String {
    let lines: Vec<&str> = text.lines().filter(|line| !line.trim().is_empty()).collect();
    lines[lines.len().saturating_sub(5)..].join("\n")
}

fn executable(program: &Path) -> Result<(), String> {
    let missing = |error: io::Error| format!("{} is missing after installing: {error}", program.display());
    let metadata = std::fs::metadata(program).map_err(missing)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = metadata.permissions();
        permissions.set_mode(permissions.mode() | 0o755);
        std::fs::set_permissions(program, permissions).map_err(missing)?;
    }
    #[cfg(not(unix))]
    let _ = metadata;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_install_lands_whole_or_not_at_all() {
        let data = tempfile::tempdir().expect("data");
        let home = data.path().join("gopls");
        std::fs::create_dir_all(&home).expect("home");
        std::fs::write(home.join("old"), "").expect("old");
        let broken = settle(&home, Path::new("new"), |staging| {
            std::fs::write(staging.join("half"), "").map_err(|e| e.to_string())?;
            Err("go: exit status 1".into())
        });
        assert_eq!(broken, Err("go: exit status 1".into()));
        assert!(home.join("old").is_file());
        assert_eq!(std::fs::read_dir(data.path()).expect("list").count(), 1);
        settle(&home, Path::new("new"), |staging| std::fs::write(staging.join("new"), "").map_err(|e| e.to_string()))
            .expect("settled");
        assert!(home.join("new").is_file() && !home.join("old").exists());
        assert_eq!(std::fs::read_dir(data.path()).expect("list").count(), 1);
    }

    #[test]
    fn an_install_another_process_finished_first_is_kept_as_it_is() {
        let data = tempfile::tempdir().expect("data");
        let home = data.path().join("pyright");
        settle(&home, Path::new("bin/server"), |staging| {
            std::fs::create_dir_all(home.join("bin")).map_err(|e| e.to_string())?;
            std::fs::write(home.join("bin/server"), "theirs").map_err(|e| e.to_string())?;
            std::fs::create_dir_all(staging.join("bin")).map_err(|e| e.to_string())?;
            std::fs::write(staging.join("bin/server"), "ours").map_err(|e| e.to_string())
        })
        .expect("already installed");
        assert_eq!(std::fs::read_to_string(home.join("bin/server")).expect("program"), "theirs");
        assert_eq!(std::fs::read_dir(data.path()).expect("list").count(), 1);
    }

    #[test]
    fn a_failed_command_reports_its_last_lines() {
        assert_eq!(last_lines("a\n\nb\nc\n  \nd\ne\nf\n"), "b\nc\nd\ne\nf");
    }
}
