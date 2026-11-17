use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use crate::bounded::Keep;
use crate::{Bounds, Ended};

pub const MOST_OUTPUT: usize = 1 << 20;

#[derive(Debug, thiserror::Error)]
pub enum TypedError {
    #[error("could not start the shell: {0}")]
    NoShell(#[source] std::io::Error),
}

impl From<TypedError> for String {
    fn from(error: TypedError) -> Self {
        error.to_string()
    }
}

pub fn run(cwd: &Path, command: &str, limit: Duration) -> Result<(String, bool), TypedError> {
    let mut shell = crate::command("sh");
    shell.arg("-c").arg(format!("exec 2>&1\n{command}")).current_dir(cwd);
    let never = AtomicBool::new(false);
    let bounds = Bounds { timeout: limit, max_output: MOST_OUTPUT + 1, stop: &never, input: None };
    let finished = crate::bounded::run_keeping(&mut shell, bounds, Keep::End).map_err(TypedError::NoShell)?;
    let mut out = ending(finished.stdout);
    match finished.ended {
        Ended::Exited(status) => Ok((out.trim_end().to_owned(), !status.success())),
        Ended::TimedOut | Ended::Stopped => {
            out.push_str(&format!("\n[stopped after {} seconds]", limit.as_secs()));
            Ok((out, true))
        }
    }
}

fn ending(output: String) -> String {
    if output.len() <= MOST_OUTPUT {
        return output;
    }
    let from = (output.len() - MOST_OUTPUT..=output.len()).find(|&at| output.is_char_boundary(at)).unwrap_or(0);
    format!("[earlier output cut: these are the last {} KiB]\n{}", MOST_OUTPUT / 1024, &output[from..])
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn a_typed_command_runs_in_the_folder_and_says_whether_it_failed() {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::write(dir.path().join("a.txt"), "x").expect("write");
        let limit = Duration::from_secs(20);
        assert_eq!(run(dir.path(), "ls", limit).expect("ran"), ("a.txt".to_owned(), false));
        assert_eq!(run(dir.path(), "echo oops >&2; exit 3", limit).expect("ran"), ("oops".to_owned(), true));
    }

    #[test]
    fn output_past_the_cap_keeps_its_end_and_says_the_start_was_cut() {
        let dir = tempfile::tempdir().expect("dir");
        let floods = "head -c 2000000 /dev/zero | tr '\\0' a; echo; echo the-end";
        let (output, failed) = run(dir.path(), floods, Duration::from_secs(20)).expect("ran");
        assert!(!failed);
        assert!(output.len() <= MOST_OUTPUT + 200, "{} bytes came back", output.len());
        assert!(output.ends_with("the-end"), "the end is what the user reads");
        assert!(output.starts_with("[earlier output cut"), "{}", &output[..60]);
    }
}
