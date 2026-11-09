//! Memory shared with the project or the organization
//! (`superfik_core::memory::shared`): what the organization's server lists
//! for this workspace's project, recalled by every prompt sent from here,
//! and sharing one of this member's memories with the project.

use superfik_core::MemoryId;
use superfik_core::memory::shared::SharedMemory;
use superfik_core::project::ProjectRef;
use superfik_core::sharing::MemoryScope;
use superfik_protocol::ClientCommand;

use super::{Effect, Notice, Session};

impl Session {
    pub(super) fn project(&self) -> Option<ProjectRef> {
        self.snapshot.repository.as_ref().and_then(|repository| repository.project.clone())
    }

    pub(super) fn load_project_sessions(&self) -> Vec<Effect> {
        match (self.organization.joined(), self.workspace_root(), self.project()) {
            (true, Some(path), Some(project)) => [
                ClientCommand::LoadSessions { path, project: Some(project.clone()) },
                ClientCommand::LoadSharedMemory { project },
            ]
            .into_iter()
            .map(|command| Effect::Command(Box::new(command)))
            .collect(),
            _ => Vec::new(),
        }
    }

    pub fn may_share_memory(&self) -> bool {
        self.organization.joined() && self.project().is_some()
    }

    pub(super) fn shared_memory_arrived(&mut self, project: &ProjectRef, memories: Vec<SharedMemory>) {
        if self.project().as_ref() == Some(project) {
            self.shared_memory = memories;
        }
    }

    pub(super) fn share_memory(&mut self, id: MemoryId) -> Vec<Effect> {
        let Some(project) = self.project().filter(|_| self.organization.joined()) else { return Vec::new() };
        let Some(memory) = self.memory.memories.iter().find(|m| m.id == id).cloned() else { return Vec::new() };
        self.notice = Some(Notice::MemoryShared);
        let command = ClientCommand::ShareMemory { project, scope: MemoryScope::Project, memory };
        vec![Effect::Command(Box::new(command))]
    }
}

#[cfg(test)]
mod tests {
    use superfik_core::{Author, Member, Memory, MemoryKind, OrgId, Principal, UserId};
    use superfik_protocol::{PROTOCOL_VERSION, ServerEvent};

    use super::*;
    use crate::state::tests::commands;
    use crate::state::{Action, EditKey};

    fn rocket() -> ProjectRef {
        ProjectRef::Remote("github.com/acme/rocket".into())
    }

    fn lint() -> Memory {
        Memory {
            id: MemoryId(3),
            kind: MemoryKind::Procedural,
            text: "Run cargo xtask lint before pushing.".into(),
            tags: vec![],
            created: 1,
            origin: None,
            author: Author::Agent,
        }
    }

    fn on_rocket() -> Session {
        let mut session = Session::new(None);
        let me = Member { user: UserId::new(), org: OrgId::new() };
        let welcome = ServerEvent::Welcome { version: PROTOCOL_VERSION, principal: Principal::Member(me) };
        session.apply(Action::Server(welcome), 0.0);
        session.root = Some("/repo".into());
        let repository =
            superfik_core::Repository { root: "/repo".into(), project: Some(rocket()), ..Default::default() };
        let snapshot = superfik_core::WorkspaceSnapshot { repository: Some(repository) };
        session.apply(Action::Server(ServerEvent::Workspace(snapshot)), 0.0);
        session.memory.memories = vec![lint()];
        session
    }

    fn shared(text: &str) -> SharedMemory {
        let memory = Memory { text: text.into(), ..lint() };
        SharedMemory { scope: MemoryScope::Organization, memory, author: UserId::new(), author_name: "Olga".into() }
    }

    #[test]
    fn opening_the_project_asks_for_its_shared_memory() {
        let mut session = Session::new(None);
        let me = Member { user: UserId::new(), org: OrgId::new() };
        let welcome = ServerEvent::Welcome { version: PROTOCOL_VERSION, principal: Principal::Member(me) };
        session.apply(Action::Server(welcome), 0.0);
        session.root = Some("/repo".into());
        let repository =
            superfik_core::Repository { root: "/repo".into(), project: Some(rocket()), ..Default::default() };
        let snapshot = superfik_core::WorkspaceSnapshot { repository: Some(repository) };
        let effects = session.apply(Action::Server(ServerEvent::Workspace(snapshot)), 0.0);
        assert!(commands(&effects).iter().any(|c| matches!(c, ClientCommand::LoadSharedMemory { .. })));
    }

    #[test]
    fn every_prompt_recalls_what_the_team_shared() {
        let mut session = on_rocket();
        let memories = vec![shared("Deploys go through the release train.")];
        session.apply(Action::Server(ServerEvent::SharedMemory { project: rocket(), memories }), 0.0);
        let elsewhere = ProjectRef::Remote("github.com/acme/other".into());
        session.apply(Action::Server(ServerEvent::SharedMemory { project: elsewhere, memories: vec![] }), 0.0);
        session.apply(Action::Edit(EditKey::Insert("Ship it".into())), 0.0);
        let effects = session.apply(Action::Submit, 1.0);
        let shared = commands(&effects).iter().find_map(|c| match c {
            ClientCommand::SubmitPrompt { request, .. } => request.memory.as_ref().map(|m| m.shared.len()),
            _ => None,
        });
        assert_eq!(shared, Some(1));
    }

    #[test]
    fn a_memory_is_shared_with_the_project() {
        let mut session = on_rocket();
        let effects = session.apply(Action::ShareMemory(MemoryId(3)), 0.0);
        let sent = commands(&effects);
        assert!(matches!(sent[..], [ClientCommand::ShareMemory { scope: MemoryScope::Project, memory, .. }]
            if memory.text.starts_with("Run cargo")));
        assert_eq!(session.notice, Some(Notice::MemoryShared));
        let mut alone = Session::new(Some("/repo".into()));
        alone.memory.memories = vec![lint()];
        assert!(commands(&alone.apply(Action::ShareMemory(MemoryId(3)), 0.0)).is_empty());
    }
}
