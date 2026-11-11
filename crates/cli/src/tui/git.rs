//! `/diff` and `/diffs`: the project's changes, as git reports them.

use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Changes {
    /// The unified diff of tracked files.
    pub diff: String,
    /// Files git does not track yet (and does not ignore).
    pub untracked: Vec<String>,
}

/// Which changes to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Staged and unstaged: `git diff HEAD`.
    Uncommitted,
    Unstaged,
    Staged,
    /// Everything since the branch left the default branch, committed or not.
    Branch,
}

impl Scope {
    pub const fn next(self) -> Self {
        match self {
            Self::Uncommitted => Self::Unstaged,
            Self::Unstaged => Self::Staged,
            Self::Staged => Self::Branch,
            Self::Branch => Self::Uncommitted,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Uncommitted => "uncommitted",
            Self::Unstaged => "unstaged",
            Self::Staged => "staged",
            Self::Branch => "branch",
        }
    }
}

pub fn changes(cwd: &Path) -> Result<Changes, String> {
    changes_in(cwd, Scope::Uncommitted)
}

pub fn changes_in(cwd: &Path, scope: Scope) -> Result<Changes, String> {
    const DIFF: [&str; 3] = ["diff", "--no-color", "--no-ext-diff"];
    let diff = match scope {
        Scope::Uncommitted => git(cwd, &[&DIFF[..], &["HEAD"]].concat()).or_else(|_| staged(cwd))?,
        Scope::Unstaged => git(cwd, &DIFF)?,
        Scope::Staged => staged(cwd)?,
        Scope::Branch => git(cwd, &[&DIFF[..], &[branch_base(cwd)?.as_str()]].concat())?,
    };
    let untracked = match scope {
        Scope::Staged => String::new(),
        _ => git(cwd, &["ls-files", "--others", "--exclude-standard"])?,
    };
    Ok(Changes { diff, untracked: untracked.lines().map(str::to_owned).collect() })
}

fn staged(cwd: &Path) -> Result<String, String> {
    git(cwd, &["diff", "--cached", "--no-color", "--no-ext-diff"])
}

/// Where this branch left the default branch (`origin/HEAD`, `main` or
/// `master`), else its upstream.
fn branch_base(cwd: &Path) -> Result<String, String> {
    ["origin/HEAD", "main", "master", "@{upstream}"]
        .iter()
        .find_map(|base| git(cwd, &["merge-base", "HEAD", base]).ok())
        .map(|commit| commit.trim().to_owned())
        .ok_or_else(|| "no main, master or upstream branch to compare with".to_owned())
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
    pub(super) fn load_diffs(&self, scope: Scope) {
        let cwd = self.cwd.clone();
        self.spawn(move |_| super::app::Msg::Diffs(scope, changes_in(&cwd, scope)));
    }

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

    #[test]
    fn each_scope_shows_its_own_changes() {
        let dir = tempfile::tempdir().expect("dir");
        let cwd = dir.path();
        let commit =
            |message: &str| run(cwd, &["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qam", message]);
        run(cwd, &["init", "-q", "-b", "main"]);
        std::fs::write(cwd.join("a.txt"), "one\n").expect("write");
        run(cwd, &["add", "a.txt"]);
        commit("a");
        run(cwd, &["checkout", "-qb", "feature"]);
        std::fs::write(cwd.join("a.txt"), "two\n").expect("edit");
        commit("two");
        std::fs::write(cwd.join("a.txt"), "three\n").expect("edit");
        run(cwd, &["add", "a.txt"]);
        std::fs::write(cwd.join("a.txt"), "four\n").expect("edit");

        let unstaged = changes_in(cwd, Scope::Unstaged).expect("unstaged");
        assert!(unstaged.diff.contains("-three\n+four"), "{}", unstaged.diff);
        let staged = changes_in(cwd, Scope::Staged).expect("staged");
        assert!(staged.diff.contains("-two\n+three"), "{}", staged.diff);
        let branch = changes_in(cwd, Scope::Branch).expect("branch");
        assert!(branch.diff.contains("-one\n+four"), "everything since main: {}", branch.diff);
        assert_eq!(Scope::Uncommitted.next(), Scope::Unstaged);
        assert_eq!(Scope::Branch.next(), Scope::Uncommitted);
    }
}
