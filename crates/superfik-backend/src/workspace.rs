//! Workspace commands: snapshots (with optional GitHub enrichment), branch
//! switching and describing further repositories a conversation attaches.

use std::path::Path;

use superfik_core::BranchRef;
use superfik_protocol::ServerEvent;
use superfik_workspace::{enrich, snapshot};

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

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use crossbeam_channel::Receiver;
    use superfik_core::{PullRequest, Repository};
    use superfik_engine::EngineRegistry;
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
    impl BranchSwitcher for FakeGit {
        fn switch(&self, _root: &Path, branch: &BranchRef) -> Result<(), WorkspaceError> {
            if branch.name == "dirty" {
                return Err(WorkspaceError::Git { command: "switch dirty".into(), message: "local changes".into() });
            }
            *self.0.lock().expect("lock") = Some(branch.name.clone());
            Ok(())
        }
    }

    struct FakeGitHub;
    impl GitHubClient for FakeGitHub {
        fn repository(&self, _root: &Path) -> Option<String> {
            Some("acme/rocket".into())
        }
        fn pull_request(&self, _root: &Path) -> Option<PullRequest> {
            Some(PullRequest { number: 7, title: "Fly".into(), ..PullRequest::default() })
        }
    }

    fn services(github: Option<Arc<dyn GitHubClient>>) -> LocalServices {
        let git = Arc::new(FakeGit::default());
        let mut services = LocalServices::with_engines(EngineRegistry::new());
        services.git = git.clone();
        services.switcher = git;
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
        inspect(&services(Some(Arc::new(FakeGitHub))), &outbox, dir().path());
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
