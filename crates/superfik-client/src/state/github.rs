//! The signed-in GitHub account's repositories: what the Repository node
//! lists, and opening (or attaching) one of them, cloning it when it is not
//! on disk yet.

use std::path::{Path, PathBuf};

use superfik_core::GitHubRepository;
use superfik_protocol::ClientCommand;

use super::{Effect, Session};

pub const GITHUB_ITEMS: usize = 8;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GitHubListing {
    pub signed_in: bool,
    pub repositories: Vec<GitHubRepository>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepositoryUse {
    Open,
    Attach,
}

impl Session {
    pub fn github_signed_in(&self) -> bool {
        self.github.as_ref().is_some_and(|listing| listing.signed_in)
    }

    pub fn github_choices(&self) -> Vec<&GitHubRepository> {
        self.github.iter().flat_map(|listing| listing.repositories.iter()).take(GITHUB_ITEMS).collect()
    }

    pub fn github_attachable(&self) -> Vec<&GitHubRepository> {
        let workspace = self.snapshot.repository.as_ref().map(|r| r.slug.as_str());
        let attached = &self.conversation().repositories;
        self.github
            .iter()
            .flat_map(|listing| listing.repositories.iter())
            .filter(|repo| {
                !same_slug(Some(repo.slug.as_str()), workspace)
                    && !attached.iter().any(|a| same_slug(Some(a.slug.as_str()), Some(&repo.slug)))
            })
            .take(GITHUB_ITEMS)
            .collect()
    }

    pub fn is_github_listed(&self, path: &Path) -> bool {
        let workspace = self.snapshot.repository.as_ref().filter(|r| r.root == path).map(|r| r.slug.as_str());
        let name = path.file_name().map(|n| n.to_string_lossy());
        self.github_choices().iter().any(|repo| {
            same_slug(workspace, Some(&repo.slug))
                || name.as_deref().is_some_and(|n| n.eq_ignore_ascii_case(repo.name()))
        })
    }

    pub fn recent_choices(&self) -> Vec<&Path> {
        if self.remote {
            return Vec::new();
        }
        self.recent_workspaces.iter().map(PathBuf::as_path).filter(|path| !self.is_github_listed(path)).collect()
    }

    pub(super) fn locate_github(&mut self, index: usize, purpose: RepositoryUse) -> Vec<Effect> {
        self.popover = None;
        let choices = match purpose {
            RepositoryUse::Open => self.github_choices(),
            RepositoryUse::Attach => self.github_attachable(),
        };
        let Some(slug) = choices.get(index).map(|repo| repo.slug.clone()) else { return Vec::new() };
        let mut known = self.recent_workspaces.clone();
        if let Some(root) = self.workspace_root()
            && !known.contains(&root)
        {
            known.insert(0, root);
        }
        self.notice = Some(super::Notice::Opening(slug.clone()));
        self.locating = Some((slug.clone(), purpose));
        vec![Effect::Command(Box::new(ClientCommand::LocateRepository {
            slug,
            known,
            clone_root: self.clone_root.clone(),
        }))]
    }

    pub(super) fn repository_located(&mut self, slug: &str, path: PathBuf) -> Vec<Effect> {
        let Some((wanted, purpose)) = self.locating.take() else { return Vec::new() };
        if !same_slug(Some(&wanted), Some(slug)) {
            return Vec::new();
        }
        self.notice = None;
        match purpose {
            RepositoryUse::Open => self.open_workspace(path),
            RepositoryUse::Attach => self.add_repository_path(path),
        }
    }
}

fn same_slug(a: Option<&str>, b: Option<&str>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use superfik_core::{Repository, WorkspaceSnapshot};
    use superfik_protocol::ServerEvent;

    use super::*;
    use crate::state::{Action, PopoverKind};

    fn repo(slug: &str) -> GitHubRepository {
        GitHubRepository { slug: slug.into(), private: false }
    }

    fn session() -> Session {
        let mut session = Session::new(Some(PathBuf::from("/code/rocket")));
        session.clone_root = PathBuf::from("/w");
        session.recent_workspaces = vec!["/code/rocket".into(), "/code/site".into()];
        session.snapshot = WorkspaceSnapshot {
            repository: Some(Repository {
                root: "/code/rocket".into(),
                slug: "acme/rocket".into(),
                ..Repository::default()
            }),
        };
        session.apply(
            Action::Server(ServerEvent::GitHubRepositories {
                signed_in: true,
                repositories: vec![repo("acme/rocket"), repo("acme/site"), repo("acme/new")],
            }),
            0.0,
        );
        session
    }

    fn slugs(repos: &[&GitHubRepository]) -> Vec<String> {
        repos.iter().map(|r| r.slug.clone()).collect()
    }

    #[test]
    fn listing_is_kept_and_filtered_for_attaching() {
        let session = session();
        assert!(session.github_signed_in());
        assert_eq!(slugs(&session.github_choices()), vec!["acme/rocket", "acme/site", "acme/new"]);
        assert_eq!(slugs(&session.github_attachable()), vec!["acme/site", "acme/new"]);
        assert!(session.is_github_listed(Path::new("/code/Site")));
        assert!(session.is_github_listed(Path::new("/code/rocket")), "the workspace, by slug");
        assert!(!session.is_github_listed(Path::new("/code/other")));
        session_with_other(&session);
    }

    fn session_with_other(session: &Session) {
        let mut session = session.clone();
        session.recent_workspaces.push("/code/other".into());
        assert_eq!(session.recent_choices(), vec![Path::new("/code/other")]);
        assert_eq!(session.attachable_repositories(), vec![PathBuf::from("/code/other")]);
    }

    #[test]
    fn signed_out_listing_offers_nothing() {
        let mut session = Session::default();
        assert!(!session.github_signed_in());
        session.apply(Action::Server(ServerEvent::GitHubRepositories { signed_in: false, repositories: vec![] }), 0.0);
        assert!(!session.github_signed_in());
        assert!(session.github_choices().is_empty());
    }

    #[test]
    fn opening_a_github_repository_locates_it_then_opens_the_workspace() {
        let mut session = session();
        session.popover = Some(PopoverKind::Repository);
        let effects = session.apply(Action::OpenGitHubRepository(2), 0.0);
        assert!(matches!(
            crate::state::tests::commands(&effects).as_slice(),
            [ClientCommand::LocateRepository { slug, known, clone_root }]
                if slug == "acme/new" && known == &["/code/rocket", "/code/site"].map(PathBuf::from) && clone_root == Path::new("/w")
        ));
        assert_eq!(session.popover, None);
        assert_eq!(session.notice_text().as_deref(), Some("Opening acme/new…"));
        let effects = session.apply(
            Action::Server(ServerEvent::RepositoryLocated { slug: "acme/new".into(), path: "/w/new".into() }),
            0.0,
        );
        assert_eq!(session.root.as_deref(), Some(Path::new("/w/new")));
        assert_eq!(session.notice, None);
        assert!(
            crate::state::tests::commands(&effects).iter().any(|c| matches!(c, ClientCommand::InspectWorkspace { .. }))
        );
    }

    #[test]
    fn attaching_a_github_repository_describes_the_located_checkout() {
        let mut session = session();
        session.apply(Action::AttachGitHubRepository(0), 0.0);
        let effects = session.apply(
            Action::Server(ServerEvent::RepositoryLocated { slug: "ACME/site".into(), path: "/code/site".into() }),
            0.0,
        );
        assert!(matches!(
            crate::state::tests::commands(&effects).as_slice(),
            [ClientCommand::InspectRepository { path }] if path == Path::new("/code/site")
        ));
        assert_eq!(session.root.as_deref(), Some(Path::new("/code/rocket")), "workspace unchanged");
    }

    #[test]
    fn unexpected_or_failed_locations_are_ignored() {
        let mut session = session();
        assert!(session.apply(Action::OpenGitHubRepository(9), 0.0).is_empty());
        let located = ServerEvent::RepositoryLocated { slug: "acme/new".into(), path: "/w/new".into() };
        assert!(session.apply(Action::Server(located.clone()), 0.0).is_empty(), "nothing was requested");
        session.apply(Action::OpenGitHubRepository(2), 0.0);
        session.apply(Action::Server(ServerEvent::error("`gh repo clone acme/new` failed")), 0.0);
        assert_eq!(session.locating, None);
        assert!(session.apply(Action::Server(located), 0.0).is_empty());
    }
}
