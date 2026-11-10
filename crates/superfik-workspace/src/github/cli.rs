//! [`GitHubClient`] backed by the GitHub CLI (`gh`), which brings its own
//! authentication.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use superfik_core::PullRequest;

use super::{GitHubClient, parse, process};
use crate::executable;

pub const ENV_OVERRIDE: &str = "SUPERFIK_GH_BIN";
const TIMEOUT: Duration = Duration::from_secs(8);

pub trait GhRunner: Send + Sync {
    fn run(&self, cwd: &Path, args: &[&str]) -> Option<String>;
}

#[derive(Debug, Clone)]
pub struct SystemGh {
    program: PathBuf,
}

impl SystemGh {
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Self { program: program.into() }
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut command = superfik_process::command(&self.program);
        command
            .args(args)
            .env("GH_PROMPT_DISABLED", "1")
            .env("GH_NO_UPDATE_NOTIFIER", "1")
            .env("NO_COLOR", "1")
            .env("GIT_OPTIONAL_LOCKS", "0");
        command
    }
}

impl GhRunner for SystemGh {
    fn run(&self, cwd: &Path, args: &[&str]) -> Option<String> {
        process::output_within(self.command(args), cwd, TIMEOUT)
    }
}

#[derive(Debug, Clone)]
pub struct GhCli<R = SystemGh> {
    runner: R,
}

impl GhCli<SystemGh> {
    pub fn detect() -> Option<Self> {
        executable::locate("gh", ENV_OVERRIDE).map(|program| Self::new(SystemGh::new(program)))
    }
}

impl<R: GhRunner> GhCli<R> {
    pub fn new(runner: R) -> Self {
        Self { runner }
    }
}

impl<R: GhRunner + 'static> GitHubClient for GhCli<R> {
    fn repository(&self, root: &Path) -> Option<String> {
        parse::repository(&self.runner.run(root, &["repo", "view", "--json", parse::REPO_FIELDS])?)
    }

    fn pull_request(&self, root: &Path) -> Option<PullRequest> {
        parse::pull_request(&self.runner.run(root, &["pr", "status", "--json", parse::PR_FIELDS])?)
    }
}

#[cfg(test)]
mod tests {
    use superfik_core::PullRequestState;

    use super::*;

    struct ScriptedGh(Vec<(&'static str, &'static str)>);
    impl GhRunner for ScriptedGh {
        fn run(&self, _cwd: &Path, args: &[&str]) -> Option<String> {
            let joined = args.join(" ");
            self.0.iter().find(|(key, _)| *key == joined).map(|(_, out)| (*out).to_owned())
        }
    }

    #[test]
    fn asks_gh_for_repository_and_pull_request() {
        let gh = GhCli::new(ScriptedGh(vec![
            ("repo view --json nameWithOwner", r#"{"nameWithOwner":"acme/rocket"}"#),
            (
                "pr status --json number,title,state,isDraft,author,headRefName,baseRefName,url,updatedAt,additions,deletions,changedFiles,reviewDecision",
                r#"{"currentBranch":{"number":9,"title":"Fly","state":"OPEN","isDraft":true}}"#,
            ),
        ]));
        assert_eq!(gh.repository(Path::new("/r")).as_deref(), Some("acme/rocket"));
        let pr = gh.pull_request(Path::new("/r")).expect("pr");
        assert_eq!((pr.number, pr.state), (9, PullRequestState::Draft));
    }

    #[test]
    fn signed_out_gh_answers_nothing() {
        let gh = GhCli::new(ScriptedGh(vec![]));
        assert_eq!(gh.repository(Path::new("/r")), None);
        assert_eq!(gh.pull_request(Path::new("/r")), None);
    }

    #[test]
    fn missing_binary_answers_nothing() {
        let gh = GhCli::new(SystemGh::new("/definitely/not/gh"));
        assert_eq!(gh.repository(Path::new("/")), None);
    }
}
