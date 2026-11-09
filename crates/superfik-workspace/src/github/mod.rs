//! Optional GitHub facts: the canonical `owner/name` and pull request of a
//! [`Repository`], the repositories of the signed-in account, and cloning.
//!
//! [`GitHubClient`] answers `None` whenever GitHub cannot be asked (no `gh`,
//! not signed in, offline, not a GitHub repository), so callers degrade to
//! git-only data without special cases.

mod cli;
mod parse;
mod process;

use std::path::{Path, PathBuf};

use superfik_core::{
    GitHubRepository, PullRequest, PullRequestDetail, PullRequestDraft, Repository, ReviewCommentDraft, ReviewVerdict,
};

use crate::error::WorkspaceError;

pub use cli::{GhCli, GhRunner, SystemGh};

pub trait GitHubClient: Send + Sync {
    fn repository(&self, root: &Path) -> Option<String>;

    fn pull_request(&self, root: &Path) -> Option<PullRequest>;

    fn repositories(&self) -> Option<Vec<GitHubRepository>>;

    fn pull_requests(&self, root: &Path) -> Option<Vec<PullRequest>>;

    fn checkout_pull_request(&self, root: &Path, number: u64) -> Result<(), WorkspaceError>;

    fn create_pull_request(&self, root: &Path, draft: &PullRequestDraft) -> Result<PullRequest, WorkspaceError>;

    fn pull_request_detail(&self, root: &Path, number: u64) -> Result<PullRequestDetail, WorkspaceError>;

    fn submit_review(
        &self,
        root: &Path,
        number: u64,
        verdict: ReviewVerdict,
        body: &str,
        comments: &[ReviewCommentDraft],
    ) -> Result<(), WorkspaceError>;

    fn resolve_thread(&self, root: &Path, thread: &str, resolved: bool) -> Result<(), WorkspaceError>;

    fn clone_repository(&self, slug: &str, destination: &Path) -> Result<(), WorkspaceError>;

    fn as_user(&self, _token: &str) -> Option<std::sync::Arc<dyn GitHubClient>> {
        None
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NoGitHub;

impl GitHubClient for NoGitHub {
    fn repository(&self, _root: &Path) -> Option<String> {
        None
    }

    fn pull_request(&self, _root: &Path) -> Option<PullRequest> {
        None
    }

    fn repositories(&self) -> Option<Vec<GitHubRepository>> {
        None
    }

    fn pull_requests(&self, _root: &Path) -> Option<Vec<PullRequest>> {
        None
    }

    fn checkout_pull_request(&self, _root: &Path, number: u64) -> Result<(), WorkspaceError> {
        Err(WorkspaceError::GitHubUnavailable(format!("#{number}")))
    }

    fn create_pull_request(&self, _root: &Path, draft: &PullRequestDraft) -> Result<PullRequest, WorkspaceError> {
        Err(WorkspaceError::GitHubUnavailable(draft.title.clone()))
    }

    fn pull_request_detail(&self, _root: &Path, number: u64) -> Result<PullRequestDetail, WorkspaceError> {
        Err(WorkspaceError::GitHubUnavailable(format!("#{number}")))
    }

    fn submit_review(
        &self,
        _root: &Path,
        number: u64,
        _verdict: ReviewVerdict,
        _body: &str,
        _comments: &[ReviewCommentDraft],
    ) -> Result<(), WorkspaceError> {
        Err(WorkspaceError::GitHubUnavailable(format!("#{number}")))
    }

    fn resolve_thread(&self, _root: &Path, thread: &str, _resolved: bool) -> Result<(), WorkspaceError> {
        Err(WorkspaceError::GitHubUnavailable(thread.to_owned()))
    }

    fn clone_repository(&self, slug: &str, _destination: &Path) -> Result<(), WorkspaceError> {
        Err(WorkspaceError::GitHubUnavailable(slug.to_owned()))
    }
}

pub fn clone_destination(root: &Path, slug: &str) -> PathBuf {
    root.join(slug.rsplit('/').next().unwrap_or(slug))
}

pub fn enrich(github: &dyn GitHubClient, repo: &mut Repository) -> bool {
    if !repo.slug.contains('/') {
        return false;
    }
    let Some(slug) = github.repository(&repo.root) else { return false };
    let pull_request = github.pull_request(&repo.root);
    let changed = slug != repo.slug || pull_request != repo.pull_request;
    repo.slug = slug;
    repo.pull_request = pull_request;
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake(Option<&'static str>, Option<PullRequest>);
    impl GitHubClient for Fake {
        fn repository(&self, _root: &Path) -> Option<String> {
            self.0.map(str::to_owned)
        }
        fn pull_request(&self, _root: &Path) -> Option<PullRequest> {
            self.1.clone()
        }
        fn repositories(&self) -> Option<Vec<GitHubRepository>> {
            None
        }
        fn pull_requests(&self, _root: &Path) -> Option<Vec<PullRequest>> {
            None
        }
        fn checkout_pull_request(&self, _root: &Path, number: u64) -> Result<(), WorkspaceError> {
            Err(WorkspaceError::GitHubUnavailable(format!("#{number}")))
        }
        fn create_pull_request(&self, _root: &Path, draft: &PullRequestDraft) -> Result<PullRequest, WorkspaceError> {
            Err(WorkspaceError::GitHubUnavailable(draft.title.clone()))
        }
        fn pull_request_detail(&self, _root: &Path, number: u64) -> Result<PullRequestDetail, WorkspaceError> {
            Err(WorkspaceError::GitHubUnavailable(format!("#{number}")))
        }
        fn submit_review(
            &self,
            _root: &Path,
            number: u64,
            _verdict: ReviewVerdict,
            _body: &str,
            _comments: &[ReviewCommentDraft],
        ) -> Result<(), WorkspaceError> {
            Err(WorkspaceError::GitHubUnavailable(format!("#{number}")))
        }
        fn resolve_thread(&self, _root: &Path, thread: &str, _resolved: bool) -> Result<(), WorkspaceError> {
            Err(WorkspaceError::GitHubUnavailable(thread.to_owned()))
        }
        fn clone_repository(&self, slug: &str, _destination: &Path) -> Result<(), WorkspaceError> {
            Err(WorkspaceError::GitHubUnavailable(slug.to_owned()))
        }
    }

    fn repo(slug: &str) -> Repository {
        Repository { root: "/r".into(), slug: slug.into(), branch: "sky".into(), ..Repository::default() }
    }

    fn pr() -> PullRequest {
        PullRequest { number: 42, title: "Sky".into(), ..PullRequest::default() }
    }

    #[test]
    fn adds_canonical_slug_and_pull_request() {
        let mut repository = repo("old/name");
        assert!(enrich(&Fake(Some("acme/name"), Some(pr())), &mut repository));
        assert_eq!(repository.slug, "acme/name");
        assert_eq!(repository.pull_request, Some(pr()));
        assert!(!enrich(&Fake(Some("acme/name"), Some(pr())), &mut repository), "unchanged");
    }

    #[test]
    fn degrades_silently_without_github() {
        let mut repository = repo("acme/name");
        assert!(!enrich(&NoGitHub, &mut repository));
        assert!(!enrich(&Fake(None, Some(pr())), &mut repository));
        assert_eq!(repository, repo("acme/name"));
    }

    #[test]
    fn clones_land_under_the_repository_name() {
        assert_eq!(
            clone_destination(Path::new("/home/me/Superfik"), "acme/rocket"),
            Path::new("/home/me/Superfik/rocket")
        );
        assert_eq!(clone_destination(Path::new("/w"), "rocket"), Path::new("/w/rocket"));
    }

    #[test]
    fn local_only_repositories_are_not_asked() {
        let mut repository = repo("sky");
        assert!(!enrich(&Fake(Some("a/b"), None), &mut repository));
        assert_eq!(repository.slug, "sky");
    }
}
