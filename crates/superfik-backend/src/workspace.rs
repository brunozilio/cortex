//! Workspace commands: snapshots (with optional GitHub enrichment), branch
//! switching, describing further repositories a conversation attaches, and
//! finding (or cloning) a GitHub repository on disk.

use std::path::{Path, PathBuf};

use superfik_core::{BranchRef, PullRequestDraft, ReviewCommentDraft, ReviewVerdict};
use superfik_protocol::ServerEvent;
use superfik_workspace::{clone_destination, enrich, snapshot};

use crate::outbox::Outbox;
use crate::services::LocalServices;

pub fn inspect(services: &LocalServices, outbox: &Outbox, path: &Path) {
    let mut snapshot = match snapshot(services.git.as_ref(), path) {
        Ok(snapshot) => snapshot,
        Err(error) => return outbox.send(ServerEvent::error(error.to_string())),
    };
    outbox.send(ServerEvent::Workspace(snapshot.clone()));
    if let Some(repository) = snapshot.repository.as_mut()
        && enrich(services.github.as_ref(), repository)
    {
        outbox.send(ServerEvent::Workspace(snapshot));
    }
}

pub fn switch_branch(services: &LocalServices, outbox: &Outbox, path: &Path, branch: &BranchRef) {
    match services.switcher.switch(path, branch) {
        Ok(()) => inspect(services, outbox, path),
        Err(error) => outbox.send(ServerEvent::error(error.to_string())),
    }
}

pub fn inspect_repository(services: &LocalServices, outbox: &Outbox, path: &Path) {
    let event = match services.git.inspect(path) {
        Ok(repository) => ServerEvent::Repository(repository),
        Err(error) => ServerEvent::error(error.to_string()),
    };
    outbox.send(event);
}

pub fn list_pull_requests(services: &LocalServices, outbox: &Outbox, path: &Path) {
    outbox.send(ServerEvent::PullRequests(services.github.pull_requests(path).unwrap_or_default()));
}

pub fn checkout_pull_request(services: &LocalServices, outbox: &Outbox, path: &Path, number: u64) {
    match services.github.checkout_pull_request(path, number) {
        Ok(()) => inspect(services, outbox, path),
        Err(error) => outbox.send(ServerEvent::error(error.to_string())),
    }
}

pub fn create_pull_request(services: &LocalServices, outbox: &Outbox, path: &Path, draft: &PullRequestDraft) {
    let created = services.pusher.push_current(path).and_then(|()| services.github.create_pull_request(path, draft));
    match created {
        Ok(pull_request) => {
            outbox.send(ServerEvent::PullRequestCreated(pull_request));
            inspect(services, outbox, path);
        }
        Err(error) => outbox.send(ServerEvent::error(error.to_string())),
    }
}

pub fn load_pull_request(services: &LocalServices, outbox: &Outbox, path: &Path, number: u64) {
    let event = match services.github.pull_request_detail(path, number) {
        Ok(detail) => ServerEvent::PullRequestLoaded(Box::new(detail)),
        Err(error) => ServerEvent::error(error.to_string()),
    };
    outbox.send(event);
}

pub fn submit_review(
    services: &LocalServices,
    outbox: &Outbox,
    path: &Path,
    number: u64,
    verdict: ReviewVerdict,
    body: &str,
    comments: &[ReviewCommentDraft],
) {
    let event = match services.github.submit_review(path, number, verdict, body, comments) {
        Ok(()) => ServerEvent::ReviewSubmitted { number },
        Err(error) => ServerEvent::error(error.to_string()),
    };
    outbox.send(event);
}

pub fn resolve_thread(services: &LocalServices, outbox: &Outbox, path: &Path, thread: &str, resolved: bool) {
    let event = match services.github.resolve_thread(path, thread, resolved) {
        Ok(()) => ServerEvent::ThreadResolved { thread: thread.to_owned(), resolved },
        Err(error) => ServerEvent::error(error.to_string()),
    };
    outbox.send(event);
}

pub fn list_github(services: &LocalServices, outbox: &Outbox) {
    let event = match services.github.repositories() {
        Some(repositories) => ServerEvent::GitHubRepositories { signed_in: true, repositories },
        None => ServerEvent::GitHubRepositories { signed_in: false, repositories: Vec::new() },
    };
    outbox.send(event);
}

pub fn locate_repository(services: &LocalServices, outbox: &Outbox, slug: &str, known: &[PathBuf], clone_root: &Path) {
    let destination = clone_destination(clone_root, slug);
    let checkout = known
        .iter()
        .map(PathBuf::as_path)
        .chain(std::iter::once(destination.as_path()))
        .find(|path| services.git.inspect(path).is_ok_and(|repo| repo.slug.eq_ignore_ascii_case(slug)));
    let located = match checkout {
        Some(path) => Ok(path.to_path_buf()),
        None => services.github.clone_repository(slug, &destination).map(|()| destination),
    };
    let event = match located {
        Ok(path) => ServerEvent::RepositoryLocated { slug: slug.to_owned(), path },
        Err(error) => ServerEvent::error(error.to_string()),
    };
    outbox.send(event);
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use crossbeam_channel::Receiver;
    use superfik_core::{GitHubRepository, PullRequest, PullRequestDetail, Repository};
    use superfik_engine::EngineRegistry;
    use superfik_workspace::BranchPusher;
    use superfik_workspace::{BranchSwitcher, GitHubClient, GitInspector, WorkspaceError};

    use super::*;

    #[derive(Default)]
    struct FakeGit(Mutex<Option<String>>);
    impl GitInspector for FakeGit {
        fn inspect(&self, root: &Path) -> Result<Repository, WorkspaceError> {
            if root.ends_with("plain") || root.ends_with("missing") {
                return Err(WorkspaceError::NotARepository(root.to_path_buf()));
            }
            let branch = self.0.lock().expect("lock").clone().unwrap_or_else(|| "main".into());
            let slug = if root.ends_with("site") { "acme/site" } else { "acme/rocket" };
            Ok(Repository { root: root.into(), slug: slug.into(), branch, ..Repository::default() })
        }
    }
    impl BranchPusher for FakeGit {
        fn push_current_with(&self, root: &Path, _env: &[(String, String)]) -> Result<(), WorkspaceError> {
            if root.ends_with("unpushable") {
                return Err(WorkspaceError::Git { command: "push".into(), message: "rejected".into() });
            }
            Ok(())
        }
    }
    impl BranchSwitcher for FakeGit {
        fn switch(&self, _root: &Path, branch: &BranchRef) -> Result<(), WorkspaceError> {
            if branch.name == "dirty" {
                return Err(WorkspaceError::Git { command: "switch dirty".into(), message: "local changes".into() });
            }
            *self.0.lock().expect("lock") = Some(branch.name.clone());
            Ok(())
        }
    }

    #[derive(Default)]
    struct FakeGitHub(Mutex<Vec<(String, PathBuf)>>, Mutex<Option<String>>);
    impl GitHubClient for FakeGitHub {
        fn repository(&self, _root: &Path) -> Option<String> {
            Some("acme/rocket".into())
        }
        fn pull_request(&self, _root: &Path) -> Option<PullRequest> {
            Some(PullRequest { number: 7, title: "Fly".into(), ..PullRequest::default() })
        }
        fn repositories(&self) -> Option<Vec<GitHubRepository>> {
            Some(vec![GitHubRepository { slug: "acme/rocket".into(), private: true }])
        }
        fn pull_requests(&self, _root: &Path) -> Option<Vec<PullRequest>> {
            Some(vec![PullRequest { number: 7, title: "Fly".into(), ..PullRequest::default() }])
        }
        fn create_pull_request(&self, _root: &Path, draft: &PullRequestDraft) -> Result<PullRequest, WorkspaceError> {
            if draft.title == "fail" {
                return Err(WorkspaceError::Gh { command: "pr create".into() });
            }
            Ok(PullRequest { number: 9, title: draft.title.clone(), ..PullRequest::default() })
        }
        fn pull_request_detail(&self, _root: &Path, number: u64) -> Result<PullRequestDetail, WorkspaceError> {
            if number == 404 {
                return Err(WorkspaceError::Gh { command: "pr view".into() });
            }
            Ok(PullRequestDetail {
                pull_request: PullRequest { number, ..PullRequest::default() },
                description: "Stars".into(),
                files: superfik_core::diff::parse("diff --git a/a.rs b/a.rs\n@@ -1 +1 @@\n-x\n+y\n"),
                comments: vec![],
            })
        }
        fn submit_review(
            &self,
            _root: &Path,
            number: u64,
            _verdict: ReviewVerdict,
            _body: &str,
            comments: &[ReviewCommentDraft],
        ) -> Result<(), WorkspaceError> {
            if comments.iter().any(|c| c.body.is_empty()) {
                return Err(WorkspaceError::Gh { command: format!("api reviews {number}") });
            }
            Ok(())
        }
        fn resolve_thread(&self, _root: &Path, thread: &str, _resolved: bool) -> Result<(), WorkspaceError> {
            if thread == "gone" { Err(WorkspaceError::Gh { command: "resolveReviewThread".into() }) } else { Ok(()) }
        }
        fn checkout_pull_request(&self, _root: &Path, number: u64) -> Result<(), WorkspaceError> {
            if number == 404 {
                return Err(WorkspaceError::Gh { command: "pr checkout 404".into() });
            }
            *self.1.lock().expect("lock") = Some(format!("pr-{number}"));
            Ok(())
        }
        fn clone_repository(&self, slug: &str, destination: &Path) -> Result<(), WorkspaceError> {
            if slug == "acme/offline" {
                return Err(WorkspaceError::Clone { slug: slug.to_owned() });
            }
            self.0.lock().expect("lock").push((slug.to_owned(), destination.to_path_buf()));
            Ok(())
        }
    }

    fn services(github: Option<Arc<dyn GitHubClient>>) -> LocalServices {
        let git = Arc::new(FakeGit::default());
        let mut services = LocalServices::with_engines(EngineRegistry::new());
        services.git = git.clone();
        services.switcher = git.clone();
        services.pusher = git;
        services.github = github.unwrap_or_else(|| Arc::new(superfik_workspace::NoGitHub));
        services
    }

    fn outbox() -> (Outbox, Receiver<ServerEvent>) {
        let (tx, rx) = crossbeam_channel::unbounded();
        (Outbox::new(tx, Arc::new(|| {})), rx)
    }

    fn branches(events: &Receiver<ServerEvent>) -> Vec<(String, Option<u64>)> {
        events
            .try_iter()
            .filter_map(|event| match event {
                ServerEvent::Workspace(snapshot) => snapshot.repository,
                _ => None,
            })
            .map(|repo| (repo.branch, repo.pull_request.map(|pr| pr.number)))
            .collect()
    }

    fn dir() -> tempfile::TempDir {
        tempfile::tempdir().expect("tempdir")
    }

    #[test]
    fn inspect_sends_git_snapshot_then_github_enrichment() {
        let (outbox, events) = outbox();
        inspect(&services(Some(Arc::new(FakeGitHub::default()))), &outbox, dir().path());
        assert_eq!(branches(&events), vec![("main".into(), None), ("main".into(), Some(7))]);
    }

    #[test]
    fn without_github_only_the_git_snapshot_is_sent() {
        let (outbox, events) = outbox();
        inspect(&services(None), &outbox, dir().path());
        assert_eq!(branches(&events), vec![("main".into(), None)]);
    }

    #[test]
    fn switching_refreshes_the_snapshot() {
        let (outbox, events) = outbox();
        switch_branch(&services(None), &outbox, dir().path(), &BranchRef::remote("spike"));
        assert_eq!(branches(&events), vec![("spike".into(), None)]);
    }

    #[test]
    fn inspect_repository_answers_repository_or_error() {
        let (outbox, events) = outbox();
        let services = services(None);
        inspect_repository(&services, &outbox, Path::new("/other"));
        inspect_repository(&services, &outbox, Path::new("/plain"));
        let events: Vec<ServerEvent> = events.try_iter().collect();
        assert!(matches!(&events[0], ServerEvent::Repository(repo) if repo.root == Path::new("/other")));
        assert!(matches!(&events[1], ServerEvent::Error { message } if message.contains("not inside a git")));
    }

    #[test]
    fn pull_requests_are_listed_or_empty_when_signed_out() {
        let (outbox, events) = outbox();
        list_pull_requests(&services(Some(Arc::new(FakeGitHub::default()))), &outbox, Path::new("/r"));
        list_pull_requests(&services(None), &outbox, Path::new("/r"));
        let events: Vec<ServerEvent> = events.try_iter().collect();
        assert!(matches!(&events[0], ServerEvent::PullRequests(prs) if prs.len() == 1));
        assert!(matches!(&events[1], ServerEvent::PullRequests(prs) if prs.is_empty()));
    }

    #[test]
    fn checking_out_a_pull_request_refreshes_the_snapshot_or_reports() {
        let (outbox, events) = outbox();
        let services = services(Some(Arc::new(FakeGitHub::default())));
        checkout_pull_request(&services, &outbox, dir().path(), 7);
        checkout_pull_request(&services, &outbox, dir().path(), 404);
        let events: Vec<ServerEvent> = events.try_iter().collect();
        assert!(matches!(&events[0], ServerEvent::Workspace(_)));
        assert!(matches!(events.last(), Some(ServerEvent::Error { message }) if message.contains("pr checkout 404")));
    }

    #[test]
    fn creating_a_pull_request_pushes_first_and_reports_it() {
        let (outbox, events) = outbox();
        let services = services(Some(Arc::new(FakeGitHub::default())));
        let draft = PullRequestDraft { title: "Sky".into(), ..PullRequestDraft::default() };
        create_pull_request(&services, &outbox, dir().path(), &draft);
        let events: Vec<ServerEvent> = events.try_iter().collect();
        assert!(matches!(&events[0], ServerEvent::PullRequestCreated(pr) if pr.number == 9 && pr.title == "Sky"));
        assert!(matches!(&events[1], ServerEvent::Workspace(_)));

        let (outbox2, events2) = self::tests::outbox();
        create_pull_request(&services, &outbox2, &dir().path().join("unpushable"), &draft);
        assert!(
            matches!(&events2.try_iter().collect::<Vec<_>>()[..], [ServerEvent::Error { message }] if message.contains("push"))
        );
        let (outbox3, events3) = self::tests::outbox();
        create_pull_request(&services, &outbox3, dir().path(), &PullRequestDraft { title: "fail".into(), ..draft });
        assert!(
            matches!(&events3.try_iter().collect::<Vec<_>>()[..], [ServerEvent::Error { message }] if message.contains("pr create"))
        );
    }

    #[test]
    fn review_commands_answer_or_report() {
        let services = services(Some(Arc::new(FakeGitHub::default())));
        let (outbox, events) = outbox();
        load_pull_request(&services, &outbox, Path::new("/r"), 5);
        load_pull_request(&services, &outbox, Path::new("/r"), 404);
        let comment = ReviewCommentDraft {
            path: "a.rs".into(),
            line: 1,
            side: superfik_core::CommentSide::New,
            body: "hm".into(),
        };
        submit_review(
            &services,
            &outbox,
            Path::new("/r"),
            5,
            ReviewVerdict::Approve,
            "",
            std::slice::from_ref(&comment),
        );
        submit_review(
            &services,
            &outbox,
            Path::new("/r"),
            5,
            ReviewVerdict::Comment,
            "",
            &[ReviewCommentDraft { body: String::new(), ..comment }],
        );
        resolve_thread(&services, &outbox, Path::new("/r"), "T1", true);
        resolve_thread(&services, &outbox, Path::new("/r"), "gone", true);
        let events: Vec<ServerEvent> = events.try_iter().collect();
        assert!(
            matches!(&events[0], ServerEvent::PullRequestLoaded(d) if d.pull_request.number == 5 && d.files.len() == 1)
        );
        assert!(matches!(&events[1], ServerEvent::Error { .. }));
        assert!(matches!(&events[2], ServerEvent::ReviewSubmitted { number: 5 }));
        assert!(matches!(&events[3], ServerEvent::Error { .. }));
        assert!(matches!(&events[4], ServerEvent::ThreadResolved { thread, resolved: true } if thread == "T1"));
        assert!(matches!(&events[5], ServerEvent::Error { .. }));
    }

    #[test]
    fn github_listing_reports_signed_out_as_empty() {
        let (outbox, events) = outbox();
        list_github(&services(Some(Arc::new(FakeGitHub::default()))), &outbox);
        list_github(&services(None), &outbox);
        let events: Vec<ServerEvent> = events.try_iter().collect();
        assert!(
            matches!(&events[0], ServerEvent::GitHubRepositories { signed_in: true, repositories } if repositories.len() == 1)
        );
        assert!(
            matches!(&events[1], ServerEvent::GitHubRepositories { signed_in: false, repositories } if repositories.is_empty())
        );
    }

    #[test]
    fn locating_prefers_known_checkouts_then_the_clone_root_then_clones() {
        let github = Arc::new(FakeGitHub::default());
        let services = services(Some(github.clone()));
        let (outbox, events) = outbox();
        let known: Vec<PathBuf> = ["/missing", "/code/site", "/code/rocket"].map(PathBuf::from).into();
        locate_repository(&services, &outbox, "acme/rocket", &known, Path::new("/w"));
        locate_repository(&services, &outbox, "ACME/site", &known, Path::new("/w"));
        locate_repository(&services, &outbox, "acme/site", &[PathBuf::from("/missing")], Path::new("/w"));
        locate_repository(&services, &outbox, "acme/new", &[], Path::new("/w"));
        locate_repository(&services, &outbox, "acme/offline", &[], Path::new("/w"));
        let located: Vec<Option<PathBuf>> = events
            .try_iter()
            .map(|event| match event {
                ServerEvent::RepositoryLocated { path, .. } => Some(path),
                _ => None,
            })
            .collect();
        assert_eq!(
            located,
            vec![
                Some("/code/rocket".into()),
                Some("/code/site".into()),
                Some("/w/site".into()),
                Some("/w/new".into()),
                None
            ]
        );
        assert_eq!(github.0.lock().expect("lock").as_slice(), &[("acme/new".to_owned(), "/w/new".into())]);
    }

    #[test]
    fn a_missing_directory_is_reported() {
        let (outbox, events) = outbox();
        inspect(&services(None), &outbox, &dir().path().join("missing"));
        assert!(
            matches!(&events.try_iter().collect::<Vec<_>>()[..], [ServerEvent::Error { message }] if message.contains("cannot read"))
        );
    }

    #[test]
    fn refused_switch_reports_the_error_only() {
        let (outbox, events) = outbox();
        switch_branch(&services(None), &outbox, dir().path(), &BranchRef::local("dirty"));
        match events.try_iter().collect::<Vec<_>>().as_slice() {
            [ServerEvent::Error { message }] => assert!(message.contains("local changes")),
            other => panic!("unexpected {other:?}"),
        }
    }
}
