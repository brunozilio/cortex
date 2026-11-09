//! Switching the checked-out branch with `git switch`.

use std::path::Path;

use superfik_core::BranchRef;
use superfik_core::repository::ORIGIN;

use super::cli::GitCli;
use super::runner::GitRunner;
use crate::error::WorkspaceError;

pub trait BranchSwitcher: Send + Sync {
    fn switch(&self, root: &Path, branch: &BranchRef) -> Result<(), WorkspaceError>;
}

impl<R: GitRunner> BranchSwitcher for GitCli<R> {
    fn switch(&self, root: &Path, branch: &BranchRef) -> Result<(), WorkspaceError> {
        let args = switch_args(branch)?;
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        self.runner().run(root, &args).map(|_| ())
    }
}

pub(crate) fn switch_args(branch: &BranchRef) -> Result<Vec<String>, WorkspaceError> {
    let name = branch.name.as_str();
    let invalid = name.is_empty()
        || name.starts_with('-')
        || name.contains("..")
        || name.chars().any(|c| c.is_whitespace() || c.is_control() || "~^:?*[\\".contains(c));
    if invalid {
        return Err(WorkspaceError::InvalidBranch(name.to_owned()));
    }
    Ok(if branch.remote {
        vec!["switch".into(), "--track".into(), format!("{ORIGIN}/{name}")]
    } else {
        vec!["switch".into(), name.to_owned()]
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    #[test]
    fn local_and_remote_arguments() {
        assert_eq!(switch_args(&BranchRef::local("dev")).expect("args"), vec!["switch", "dev"]);
        assert_eq!(
            switch_args(&BranchRef::remote("feat/sky")).expect("args"),
            vec!["switch", "--track", "origin/feat/sky"]
        );
    }

    #[test]
    fn option_like_and_malformed_names_are_rejected() {
        for name in ["", "-f", "--detach", "a b", "a..b", "a~1", "x:y"] {
            assert!(
                matches!(switch_args(&BranchRef::local(name)), Err(WorkspaceError::InvalidBranch(_))),
                "{name:?} accepted"
            );
        }
    }

    #[derive(Default)]
    struct Recording(Mutex<Vec<String>>);
    impl GitRunner for Recording {
        fn run(&self, _cwd: &Path, args: &[&str]) -> Result<String, WorkspaceError> {
            self.0.lock().expect("lock").push(args.join(" "));
            if args.contains(&"dirty") {
                return Err(WorkspaceError::Git { command: args.join(" "), message: "local changes".into() });
            }
            Ok(String::new())
        }
    }

    #[test]
    fn runs_git_switch_and_surfaces_failures() {
        let git = GitCli::new(Recording::default());
        git.switch(Path::new("/r"), &BranchRef::remote("spike")).expect("switch");
        let error = git.switch(Path::new("/r"), &BranchRef::local("dirty")).expect_err("refused");
        assert!(error.to_string().contains("local changes"));
        assert_eq!(*git.runner().0.lock().expect("lock"), vec!["switch --track origin/spike", "switch dirty"]);
    }
}
