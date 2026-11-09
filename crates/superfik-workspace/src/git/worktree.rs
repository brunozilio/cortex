//! A separate checkout on a new branch, for work that must not touch the
//! member's own (a routine that edits files).

use std::path::Path;

use super::cli::GitCli;
use super::runner::GitRunner;
use crate::error::WorkspaceError;

pub trait Worktrees: Send + Sync {
    fn add(&self, repo: &Path, path: &Path, branch: &str) -> Result<(), WorkspaceError>;

    fn remove_if_untouched(&self, repo: &Path, path: &Path, branch: &str) -> Result<bool, WorkspaceError>;
}

fn check(branch: &str) -> Result<(), WorkspaceError> {
    if branch.is_empty() || branch.starts_with('-') || branch.contains("..") || branch.contains(' ') {
        return Err(WorkspaceError::InvalidBranch(branch.to_owned()));
    }
    Ok(())
}

impl<R: GitRunner> Worktrees for GitCli<R> {
    fn add(&self, repo: &Path, path: &Path, branch: &str) -> Result<(), WorkspaceError> {
        check(branch)?;
        let path = path.to_string_lossy();
        self.runner().run(repo, &["worktree", "add", "-b", branch, "--", &path, "HEAD"]).map(|_| ())
    }

    fn remove_if_untouched(&self, repo: &Path, path: &Path, branch: &str) -> Result<bool, WorkspaceError> {
        check(branch)?;
        let git = self.runner();
        if !git.run(path, &["status", "--porcelain"])?.trim().is_empty() {
            return Ok(false);
        }
        let others = format!("--exclude=refs/heads/{branch}");
        if !git.run(repo, &["rev-list", branch, "--not", &others, "--branches"])?.trim().is_empty() {
            return Ok(false);
        }
        let shown = path.to_string_lossy();
        git.run(repo, &["worktree", "remove", "--", &shown])?;
        git.run(repo, &["branch", "-D", branch])?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    struct Recorder(Mutex<Vec<String>>, &'static str, &'static str);

    impl GitRunner for Recorder {
        fn run(&self, cwd: &Path, args: &[&str]) -> Result<String, WorkspaceError> {
            self.0.lock().expect("lock").push(format!("{} :: {}", cwd.display(), args.join(" ")));
            Ok(match args.first() {
                Some(&"status") => self.1,
                Some(&"rev-list") => self.2,
                _ => "",
            }
            .to_owned())
        }
    }

    fn git(status: &'static str, commits: &'static str) -> GitCli<Recorder> {
        GitCli::new(Recorder(Mutex::new(Vec::new()), status, commits))
    }

    fn ran(git: &GitCli<Recorder>) -> Vec<String> {
        git.runner().0.lock().expect("lock").clone()
    }

    #[test]
    fn an_untouched_worktree_goes_with_its_branch() {
        let git = git("", "");
        let removed = git.remove_if_untouched(Path::new("/repo"), Path::new("/wt/a"), "routine/a");
        assert_eq!(removed.ok(), Some(true));
        assert_eq!(
            ran(&git),
            [
                "/wt/a :: status --porcelain",
                "/repo :: rev-list routine/a --not --exclude=refs/heads/routine/a --branches",
                "/repo :: worktree remove -- /wt/a",
                "/repo :: branch -D routine/a",
            ]
        );
    }

    #[test]
    fn a_worktree_with_changes_or_commits_stays() {
        let dirty = git(" M src/lib.rs\n", "");
        assert_eq!(dirty.remove_if_untouched(Path::new("/repo"), Path::new("/wt"), "routine/a").ok(), Some(false));
        assert_eq!(ran(&dirty).len(), 1);
        let committed = git("", "abc123\n");
        assert_eq!(committed.remove_if_untouched(Path::new("/repo"), Path::new("/wt"), "routine/a").ok(), Some(false));
        assert!(!ran(&committed).iter().any(|c| c.contains("remove")));
    }

    #[test]
    fn adds_a_worktree_on_a_new_branch_from_head() {
        let git = git("", "");
        let added = git.add(Path::new("/repo"), Path::new("/data/wt/upkeep"), "routine/upkeep-20261026-0900");
        assert!(added.is_ok());
        assert_eq!(
            git.runner().0.lock().expect("lock").as_slice(),
            ["/repo :: worktree add -b routine/upkeep-20261026-0900 -- /data/wt/upkeep HEAD"]
        );
    }

    #[test]
    fn refuses_names_git_would_read_as_options() {
        let git = git("", "");
        assert!(git.add(Path::new("/repo"), Path::new("/wt"), "--force").is_err());
        assert!(git.runner().0.lock().expect("lock").is_empty());
    }
}
