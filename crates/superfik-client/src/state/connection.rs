//! Which server the app talks to (ADR 0010): this computer's own, as the
//! local owner, or an organization's, as a member. The server runs
//! everything; the app only learns from the greeting where workspaces come
//! from. Changing servers leaves the workspace, since its folders belong to
//! the server that was.

use superfik_core::Principal;
use superfik_protocol::ClientCommand::{GetPolicy, ListLibrary, ListMembers, ListTeams};
use superfik_protocol::LinkState;

use super::{Effect, Notice, OrganizationState, Session};

impl Session {
    pub(super) fn welcomed(&mut self, principal: Principal) -> Vec<Effect> {
        let me = match principal {
            Principal::Member(member) => Some(member),
            _ => None,
        };
        let mut effects = Vec::new();
        if me != self.organization.me || me.is_some() != self.remote {
            self.remote = me.is_some();
            self.organization = OrganizationState::default();
            self.server_link.settings = None;
            self.server_link.code_shown = None;
            effects.extend(self.leave_workspace());
            effects.push(Effect::Command(Box::new(superfik_protocol::ClientCommand::LoadSettings)));
        }
        if let Some(member) = me {
            self.organization.me = Some(member);
            let asked = [ListLibrary, GetPolicy, ListTeams, ListMembers];
            effects.extend(asked.into_iter().map(|c| Effect::Command(Box::new(c))));
            if self.server_pane_open() {
                effects.extend(self.server_pane_shown());
            }
        }
        effects
    }

    pub(super) fn linked(&mut self, state: LinkState) {
        match state {
            LinkState::Lost => self.notice = Some(Notice::Reconnecting),
            LinkState::Restored if self.notice == Some(Notice::Reconnecting) => self.notice = None,
            LinkState::Restored => {}
            LinkState::SignedOut => self.notice = Some(Notice::ServerSignedOut),
        }
    }

    pub fn signs_in_to_github(&self) -> bool {
        self.remote
    }

    pub fn opens_folders(&self) -> bool {
        !self.remote
    }

    fn leave_workspace(&mut self) -> Vec<Effect> {
        if self.root.is_none() {
            return Vec::new();
        }
        let effects = self.cancel_all();
        let recents = std::mem::take(&mut self.recent_workspaces);
        self.reset_workspace(None, recents);
        effects
    }
}

#[cfg(test)]
mod tests {
    use superfik_core::{Member, OrgId, UserId};
    use superfik_protocol::{ClientCommand, PROTOCOL_VERSION, ServerEvent};

    use super::*;
    use crate::state::tests::commands;
    use crate::state::{Action, PopoverKind};

    fn welcome(session: &mut Session, principal: Principal) -> Vec<Effect> {
        session.apply(Action::Server(ServerEvent::Welcome { version: PROTOCOL_VERSION, principal }), 0.0)
    }

    fn member() -> Principal {
        Principal::Member(Member { user: UserId::new(), org: OrgId::new() })
    }

    #[test]
    fn an_organizations_server_leaves_this_computers_folder() {
        let mut session = Session::new(Some("/Users/olga/rocket".into()));
        let effects = welcome(&mut session, member());
        assert!(session.remote);
        assert_eq!(session.root, None, "a folder here means nothing there");
        assert!(commands(&effects).iter().any(|c| matches!(c, ClientCommand::GetPolicy)));
        assert!(commands(&effects).iter().any(|c| matches!(c, ClientCommand::LoadSettings)), "its own settings");
        assert!(session.organization.me.is_some(), "who the app is outlives the workspace");
    }

    #[test]
    fn another_member_or_back_home_forgets_the_organization_and_its_folders() {
        let mut session = Session::new(None);
        welcome(&mut session, member());
        session.root = Some("/srv/checkouts/u1/rocket".into());
        welcome(&mut session, member());
        assert_eq!(session.root, None, "another organization's checkout");
        session.root = Some("/srv/checkouts/u2/rocket".into());
        welcome(&mut session, Principal::LocalOwner);
        assert_eq!((session.root.clone(), session.organization.me, session.remote), (None, None, false));
        assert!(session.opens_folders());
    }

    #[test]
    fn the_same_member_welcomed_again_keeps_the_workspace() {
        let mut session = Session::new(None);
        let me = member();
        welcome(&mut session, me.clone());
        session.root = Some("/srv/checkouts/u1/rocket".into());
        welcome(&mut session, me);
        assert!(session.root.is_some());
    }

    #[test]
    fn a_dropped_link_says_so_until_it_is_back_or_needs_a_sign_in() {
        let mut session = Session::new(None);
        let link = |session: &mut Session, state| session.apply(Action::Server(ServerEvent::Link(state)), 0.0);
        link(&mut session, LinkState::Lost);
        assert_eq!(session.notice, Some(Notice::Reconnecting));
        link(&mut session, LinkState::Restored);
        assert_eq!(session.notice, None);
        link(&mut session, LinkState::Lost);
        link(&mut session, LinkState::SignedOut);
        assert_eq!(session.notice, Some(Notice::ServerSignedOut));
        link(&mut session, LinkState::Restored);
        assert_eq!(session.notice, Some(Notice::ServerSignedOut), "only its own notice is cleared");
    }

    #[test]
    fn on_an_organizations_server_files_are_sent_and_join_once_kept() {
        let mut session = Session::new(None);
        welcome(&mut session, member());
        let sky = std::path::PathBuf::from("/Users/olga/sky.png");
        let effects = session.apply(Action::AddReferences(vec![sky.clone()]), 0.0);
        assert!(matches!(&effects[..], [Effect::Upload(paths)] if *paths == [sky.clone()]));
        assert!(session.conversation().references.is_empty(), "not until the server keeps it");
        assert!(session.apply(Action::AddReferences(vec![sky.clone()]), 0.0).is_empty(), "sent once");
        let kept = std::path::PathBuf::from("/srv/checkouts/u1/uploads/1/sky.png");
        let uploaded = ServerEvent::Uploaded { from: sky.clone(), path: kept.clone() };
        session.apply(Action::Server(uploaded), 0.0);
        let conversation = session.conversation();
        assert_eq!(conversation.references, std::slice::from_ref(&kept));
        assert_eq!(conversation.preview(&kept), sky.as_path(), "its tile shows the original");
    }

    #[test]
    fn on_an_organizations_server_github_is_signed_in_to_through_it() {
        let mut session = Session::new(None);
        assert!(!session.signs_in_to_github(), "gh signs in on this computer");
        welcome(&mut session, member());
        assert!(session.signs_in_to_github());
        let effects = session.apply(Action::SignInToGitHub, 0.0);
        assert!(commands(&effects).iter().any(|c| matches!(c, ClientCommand::SignInToGitHub)));
        let code = ServerEvent::GitHubCode { code: "ABCD-1234".into(), url: "https://github.com/login/device".into() };
        let effects = session.apply(Action::Server(code), 0.0);
        assert_eq!(session.notice, Some(Notice::GitHubCode("ABCD-1234".into())));
        assert!(effects.iter().any(|e| matches!(e, Effect::OpenUrl(url) if url.contains("github.com"))));
        let effects = session.apply(Action::Server(ServerEvent::GitHubSignedIn { login: Some("olga".into()) }), 0.0);
        assert_eq!(session.notice, None);
        assert!(commands(&effects).iter().any(|c| matches!(c, ClientCommand::ListGitHubRepositories)));
    }

    #[test]
    fn this_computers_server_keeps_its_folder() {
        let mut session = Session::new(Some("/Users/olga/rocket".into()));
        welcome(&mut session, Principal::LocalOwner);
        assert!(!session.remote);
        assert_eq!(session.root.as_deref(), Some(std::path::Path::new("/Users/olga/rocket")));
    }

    #[test]
    fn on_an_organizations_server_a_workspace_is_picked_from_its_projects() {
        let mut session = Session::new(None);
        session.recent_workspaces = vec!["/Users/olga/rocket".into()];
        welcome(&mut session, member());
        let effects = session.apply(Action::RequestWorkspace, 0.0);
        assert_eq!(session.popover, Some(PopoverKind::Repository));
        assert!(commands(&effects).iter().any(|c| matches!(c, ClientCommand::ListGitHubRepositories)));
        assert!(session.recent_choices().is_empty(), "this computer's folders are not offered");
        session.apply(Action::RequestRepository, 0.0);
        assert_eq!(session.popover, Some(PopoverKind::AttachRepository));
    }
}
