//! Publishing the checked-out branch with `git push`.

use std::path::Path;

use superfik_core::repository::ORIGIN;

use super::cli::GitCli;
use super::runner::GitRunner;
use crate::error::WorkspaceError;

pub trait BranchPusher: Send + Sync {
    fn push_current(&self, root: &Path) -> Result<(), WorkspaceError> {
        self.push_current_with(root, &[])
    }

    fn push_current_with(&self, root: &Path, env: &[(String, String)]) -> Result<(), WorkspaceError>;
}

impl<R: GitRunner> BranchPusher for GitCli<R> {
    fn push_current_with(&self, root: &Path, env: &[(String, String)]) -> Result<(), WorkspaceError> {
        self.runner().run_with(root, &["push", "--set-upstream", ORIGIN, "HEAD"], env).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    struct Recorder(Mutex<Vec<String>>);
    impl GitRunner for Recorder {
        fn run(&self, _cwd: &Path, args: &[&str]) -> Result<String, WorkspaceError> {
            self.0.lock().expect("lock").push(args.join(" "));
            if args.contains(&"HEAD") { Ok(String::new()) } else { Err(WorkspaceError::InvalidBranch("x".into())) }
        }
    }

    #[test]
    fn pushes_head_with_upstream() {
        let git = GitCli::new(Recorder(Mutex::new(Vec::new())));
        assert!(git.push_current(Path::new("/r")).is_ok());
        assert_eq!(git.runner().0.lock().expect("lock").as_slice(), ["push --set-upstream origin HEAD"]);
    }
}
