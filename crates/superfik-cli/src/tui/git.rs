//! `/diff`: the project's uncommitted changes, as git reports them.

use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Changes {
    /// `git diff HEAD`: staged and unstaged changes to tracked files.
    pub diff: String,
    /// Files git does not track yet (and does not ignore).
    pub untracked: Vec<String>,
}

pub fn changes(cwd: &Path) -> Result<Changes, String> {
    let diff = git(cwd, &["diff", "HEAD", "--no-color", "--no-ext-diff"])
        .or_else(|_| git(cwd, &["diff", "--cached", "--no-color", "--no-ext-diff"]))?;
    let untracked = git(cwd, &["ls-files", "--others", "--exclude-standard"])?;
    Ok(Changes { diff, untracked: untracked.lines().map(str::to_owned).collect() })
}

fn git(cwd: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(["-c", "core.quotepath=off"])
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|error| format!("git: {error}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_owned())
    }
}

impl super::app::App {
    pub(super) fn changes(&mut self, changes: Result<Changes, String>) {
        match changes {
            Ok(Changes { diff, untracked }) if diff.trim().is_empty() && untracked.is_empty() => {
                self.transcript.notice("No uncommitted changes.");
            }
            Ok(Changes { diff, untracked }) => self.transcript.push(super::transcript::Entry::Diff { diff, untracked }),
            Err(error) => self.transcript.error(format!("/diff needs a git repository: {error}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(cwd: &Path, args: &[&str]) {
        let status = Command::new("git").args(args).current_dir(cwd).output().expect("git");
        assert!(status.status.success(), "{args:?}: {}", String::from_utf8_lossy(&status.stderr));
    }

    #[test]
    fn changes_are_the_diff_against_head_and_the_new_files() {
        let dir = tempfile::tempdir().expect("dir");
        let cwd = dir.path();
        run(cwd, &["init", "-q"]);
        std::fs::write(cwd.join("a.txt"), "one\n").expect("write");
        run(cwd, &["add", "a.txt"]);
        run(cwd, &["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", "a"]);
        std::fs::write(cwd.join("a.txt"), "two\n").expect("edit");
        std::fs::write(cwd.join("new.txt"), "new\n").expect("new");
        let found = changes(cwd).expect("changes");
        assert!(found.diff.contains("-one\n+two"), "{}", found.diff);
        assert_eq!(found.untracked, ["new.txt"]);
        let outside = tempfile::tempdir().expect("outside");
        assert!(changes(outside.path()).is_err(), "not a repository");
    }
}
