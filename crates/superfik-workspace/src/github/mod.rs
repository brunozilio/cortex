//! Optional GitHub facts: the canonical `owner/name` and pull request of a
//! [`Repository`].
//!
//! [`GitHubClient`] answers `None` whenever GitHub cannot be asked (no `gh`,
//! not signed in, offline, not a GitHub repository), so callers degrade to
//! git-only data without special cases.

mod cli;
mod parse;
mod process;

use std::path::Path;

use superfik_core::{PullRequest, Repository};

pub use cli::{GhCli, GhRunner, SystemGh};

pub trait GitHubClient: Send + Sync {
    fn repository(&self, root: &Path) -> Option<String>;

    fn pull_request(&self, root: &Path) -> Option<PullRequest>;
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
    fn local_only_repositories_are_not_asked() {
        let mut repository = repo("sky");
        assert!(!enrich(&Fake(Some("a/b"), None), &mut repository));
        assert_eq!(repository.slug, "sky");
    }
}
