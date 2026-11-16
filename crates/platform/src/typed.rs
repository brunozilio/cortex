use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use crate::{Bounds, Ended};

pub fn run(cwd: &Path, command: &str, limit: Duration) -> Result<(String, bool), String> {
    let mut shell = crate::command("sh");
    shell.arg("-c").arg(format!("exec 2>&1\n{command}")).current_dir(cwd);
    let never = AtomicBool::new(false);
    let bounds = Bounds { timeout: limit, max_output: usize::MAX, stop: &never, input: None };
    let finished =
        crate::bounded::run(&mut shell, bounds).map_err(|error| format!("could not start the shell: {error}"))?;
    let mut out = finished.stdout;
    match finished.ended {
        Ended::Exited(status) => Ok((out.trim_end().to_owned(), !status.success())),
        Ended::TimedOut | Ended::Stopped => {
            out.push_str(&format!("\n[stopped after {} seconds]", limit.as_secs()));
            Ok((out, true))
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn a_typed_command_runs_in_the_folder_and_says_whether_it_failed() {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::write(dir.path().join("a.txt"), "x").expect("write");
        let limit = Duration::from_secs(20);
        assert_eq!(run(dir.path(), "ls", limit), Ok(("a.txt".to_owned(), false)));
        assert_eq!(run(dir.path(), "echo oops >&2; exit 3", limit), Ok(("oops".to_owned(), true)));
    }
}
