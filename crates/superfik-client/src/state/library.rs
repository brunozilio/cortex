//! The organization's library in the app (`superfik_core::library`):
//! keeping this desktop's copy laid out as the `org` plugin, and
//! `/publish <name>`, which reads one of the member's own skills or agents
//! on the device and publishes it to the organization.

use superfik_core::library::{self, LibraryItem};
use superfik_protocol::ClientCommand;

use super::{Effect, Notice, Session};

fn command(command: ClientCommand) -> Effect {
    Effect::Command(Box::new(command))
}

impl Session {
    pub(super) fn library_listed(&mut self, items: Vec<LibraryItem>) -> Vec<Effect> {
        self.organization.library = items.clone();
        vec![command(ClientCommand::InstallLibrary { items })]
    }

    pub(super) fn run_publish(&mut self) -> Option<Vec<Effect>> {
        let name = library::publish_target(self.conversation().composer.text())?;
        self.conversation_mut().composer.take();
        if !self.manages_organization() {
            self.notice = Some(Notice::PublishNotAllowed);
            return Some(Vec::new());
        }
        let Some(path) = self.workspace_root() else {
            self.notice = Some(Notice::ChooseRepository);
            return Some(Vec::new());
        };
        Some(vec![command(ClientCommand::ReadLibraryItem { path, name })])
    }

    pub(super) fn library_item_read(&mut self, item: LibraryItem) -> Vec<Effect> {
        self.notice = Some(Notice::Published(item.name.clone()));
        vec![command(ClientCommand::PublishToLibrary { item })]
    }

    pub(super) fn remove_from_library(&mut self, index: usize) -> Vec<Effect> {
        let Some(item) = self.organization.library.get(index) else { return Vec::new() };
        vec![command(ClientCommand::RemoveFromLibrary { kind: item.kind, name: item.name.clone() })]
    }
}

#[cfg(test)]
mod tests {
    use superfik_core::access::{Colleague, OrgRole};
    use superfik_core::library::LibraryKind;
    use superfik_core::{Member, OrgId, Principal, UserId};
    use superfik_protocol::{PROTOCOL_VERSION, ServerEvent};

    use super::*;
    use crate::state::tests::commands;
    use crate::state::{Action, EditKey};

    fn member(role: OrgRole) -> Session {
        let mut session = Session::new(None);
        let me = Member { user: UserId::new(), org: OrgId::new() };
        let welcome = ServerEvent::Welcome { version: PROTOCOL_VERSION, principal: Principal::Member(me) };
        session.apply(Action::Server(welcome), 0.0);
        session.root = Some("/repo".into());
        let colleague = Colleague { user: me.user, name: "olga".into(), role, pending: false };
        session.apply(Action::Server(ServerEvent::Members(vec![colleague])), 0.0);
        session
    }

    fn release() -> LibraryItem {
        LibraryItem { kind: LibraryKind::Skill, name: "release".into(), body: "---".into(), author_name: String::new() }
    }

    #[test]
    fn joining_asks_for_the_library_and_lays_it_out_when_it_comes() {
        let mut session = Session::new(Some("/repo".into()));
        let me = Member { user: UserId::new(), org: OrgId::new() };
        let welcome = ServerEvent::Welcome { version: PROTOCOL_VERSION, principal: Principal::Member(me) };
        let effects = session.apply(Action::Server(welcome), 0.0);
        assert!(commands(&effects).iter().any(|c| matches!(c, ClientCommand::ListLibrary)));
        let effects = session.apply(Action::Server(ServerEvent::Library(vec![release()])), 0.0);
        assert!(matches!(commands(&effects)[..], [ClientCommand::InstallLibrary { items }] if items.len() == 1));
    }

    #[test]
    fn an_admin_publishes_by_name_through_the_device() {
        let mut session = member(OrgRole::Admin);
        session.apply(Action::Edit(EditKey::Insert("/publish release".into())), 0.0);
        let effects = session.apply(Action::Submit, 1.0);
        assert!(matches!(commands(&effects)[..], [ClientCommand::ReadLibraryItem { name, .. }] if name == "release"));
        assert!(session.conversation().turns.is_empty(), "nothing went to the model");
        let effects = session.apply(Action::Server(ServerEvent::LibraryItemRead(release())), 2.0);
        assert!(matches!(commands(&effects)[..], [ClientCommand::PublishToLibrary { .. }]));
        assert_eq!(session.notice, Some(Notice::Published("release".into())));
    }

    #[test]
    fn a_member_is_told_only_admins_publish() {
        let mut session = member(OrgRole::Member);
        session.apply(Action::Edit(EditKey::Insert("/publish release".into())), 0.0);
        assert!(commands(&session.apply(Action::Submit, 1.0)).is_empty());
        assert_eq!(session.notice, Some(Notice::PublishNotAllowed));
    }
}
