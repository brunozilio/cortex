//! Workspace actions that go through the backend: switching branches and
//! describing repositories to attach.

use std::path::PathBuf;

use superfik_core::BranchRef;
use superfik_protocol::ClientCommand;

use super::{Effect, PopoverKind, Session};

impl Session {
    pub fn workspace_root(&self) -> Option<PathBuf> {
        self.snapshot.repository.as_ref().map(|repo| repo.root.clone()).or_else(|| self.root.clone())
    }

    pub(super) fn select_branch(&mut self, branch: BranchRef) -> Vec<Effect> {
        self.popover = None;
        if !branch.remote && self.branch.as_deref() == Some(branch.name.as_str()) {
            return Vec::new();
        }
        if self.any_running() {
            self.notice = Some(super::Notice::BusySwitch);
            return Vec::new();
        }
        let Some(path) = self.workspace_root() else { return Vec::new() };
        vec![Effect::Command(Box::new(ClientCommand::SwitchBranch { path, branch }))]
    }

    pub fn attachable_repositories(&self) -> Vec<PathBuf> {
        if self.remote {
            return Vec::new();
        }
        let root = self.workspace_root();
        let attached = &self.conversation().repositories;
        self.recent_workspaces
            .iter()
            .filter(|path| root.as_ref() != Some(path) && !attached.iter().any(|r| &r.root == *path))
            .filter(|path| !self.is_github_listed(path))
            .cloned()
            .collect()
    }

    pub(super) fn request_repository(&mut self) -> Vec<Effect> {
        self.popover = None;
        if self.remote {
            return self.toggle_popover(PopoverKind::AttachRepository);
        }
        vec![Effect::PickRepository]
    }

    pub(super) fn add_repository_path(&mut self, path: PathBuf) -> Vec<Effect> {
        self.popover = None;
        vec![Effect::Command(Box::new(ClientCommand::InspectRepository { path }))]
    }
}

#[cfg(test)]
mod tests {
    use superfik_core::{Repository, WorkspaceSnapshot};
    use superfik_protocol::ServerEvent;

    use super::*;
    use crate::state::{Action, EditKey, PopoverKind};

    fn session() -> Session {
        let mut session = Session::new(Some(PathBuf::from("/r/src")));
        let repository = Repository {
            root: "/r".into(),
            slug: "acme/rocket".into(),
            branch: "main".into(),
            branches: vec!["dev".into(), "main".into()],
            remote_branches: vec!["spike".into()],
            ..Repository::default()
        };
        session.apply(Action::Server(ServerEvent::Workspace(WorkspaceSnapshot { repository: Some(repository) })), 0.0);
        session
    }

    fn command(effects: &[Effect]) -> Option<&ClientCommand> {
        match effects {
            [Effect::Command(command)] => Some(command),
            _ => None,
        }
    }

    #[test]
    fn selecting_another_branch_asks_the_backend_to_switch() {
        let mut session = session();
        session.popover = Some(PopoverKind::Branch);
        let effects = session.apply(Action::SelectBranch(BranchRef::remote("spike")), 0.0);
        assert_eq!(
            command(&effects),
            Some(&ClientCommand::SwitchBranch { path: "/r".into(), branch: BranchRef::remote("spike") })
        );
        assert_eq!(session.popover, None);
        assert_eq!(session.branch.as_deref(), Some("main"), "unchanged until the snapshot arrives");
    }

    #[test]
    fn selecting_the_current_branch_does_nothing() {
        let mut session = session();
        assert!(session.apply(Action::SelectBranch(BranchRef::local("main")), 0.0).is_empty());
    }

    #[test]
    fn switching_is_refused_while_running() {
        let mut session = session();
        session.apply(Action::Edit(EditKey::Insert("go".into())), 0.0);
        session.apply(Action::Submit, 0.0);
        assert!(session.apply(Action::SelectBranch(BranchRef::local("dev")), 0.0).is_empty());
        assert_eq!(session.notice, Some(crate::state::Notice::BusySwitch));
    }

    fn other(root: &str) -> Repository {
        Repository {
            root: root.into(),
            slug: format!("acme/{}", root.trim_start_matches('/')),
            ..Repository::default()
        }
    }

    #[test]
    fn a_chosen_repository_is_described_then_attached_once() {
        let mut session = session();
        session.popover = Some(PopoverKind::AttachRepository);
        let effects = session.apply(Action::AddRepositoryPath("/engine".into()), 0.0);
        assert_eq!(command(&effects), Some(&ClientCommand::InspectRepository { path: "/engine".into() }));
        assert_eq!(session.popover, None);
        session.apply(Action::Server(ServerEvent::Repository(other("/engine"))), 0.0);
        session.apply(Action::Server(ServerEvent::Repository(other("/engine"))), 0.0);
        assert_eq!(session.conversation().repositories, vec![other("/engine")]);
    }

    #[test]
    fn the_workspace_itself_is_never_attached() {
        let mut session = session();
        session.apply(Action::Server(ServerEvent::Repository(other("/r"))), 0.0);
        assert!(session.conversation().repositories.is_empty());
    }

    #[test]
    fn attached_repositories_can_be_removed_and_travel_with_the_prompt() {
        let mut session = session();
        session.apply(Action::Server(ServerEvent::Repository(other("/engine"))), 0.0);
        session.apply(Action::Server(ServerEvent::Repository(other("/docs"))), 0.0);
        session.apply(Action::RemoveRepository(0), 0.0);
        session.apply(Action::RemoveRepository(7), 0.0);
        assert_eq!(session.conversation().repositories, vec![other("/docs")]);
        session.apply(Action::Edit(EditKey::Insert("go".into())), 0.0);
        let effects = session.apply(Action::Submit, 0.0);
        let request = effects.iter().find_map(|effect| match effect {
            Effect::Command(command) => match command.as_ref() {
                ClientCommand::SubmitPrompt { request, .. } => Some(request.clone()),
                _ => None,
            },
            _ => None,
        });
        assert_eq!(request.map(|r| r.repositories), Some(vec![other("/docs")]));
    }

    #[test]
    fn attachable_repositories_skip_the_workspace_and_attached_ones() {
        let mut session = session();
        session.recent_workspaces = vec!["/r".into(), "/engine".into(), "/docs".into()];
        session.apply(Action::Server(ServerEvent::Repository(other("/engine"))), 0.0);
        assert_eq!(session.attachable_repositories(), vec![PathBuf::from("/docs")]);
        assert!(matches!(&session.apply(Action::RequestRepository, 0.0)[..], [Effect::PickRepository]));
    }

    #[test]
    fn refreshed_snapshot_moves_the_branch_and_errors_become_notices() {
        let mut session = session();
        let mut snapshot = session.snapshot.clone();
        if let Some(repo) = snapshot.repository.as_mut() {
            repo.branch = "dev".into();
        }
        session.apply(Action::Server(ServerEvent::Workspace(snapshot)), 0.0);
        assert_eq!(session.branch.as_deref(), Some("dev"));
        session.apply(Action::Server(ServerEvent::error("`git switch main` failed: local changes")), 0.0);
        assert!(session.notice_text().as_deref().is_some_and(|n| n.contains("local changes")));
    }
}
