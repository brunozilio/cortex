//! The team this app belongs to, as its server last told it: who the users
//! are and who this app signed in as. The app reaches the server from
//! Settings › Server (`server_link`), where its admins also add and remove
//! users.

use superfik_core::Member;
use superfik_core::access::{Colleague, OrgRole};
use superfik_protocol::ClientCommand;

use super::{Effect, Session};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrgAction {
    AnswerDelegated(usize, bool),
    RemoveFromLibrary(usize),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct OrganizationState {
    pub me: Option<Member>,
    pub members: Vec<Colleague>,
    pub delegated: Vec<superfik_core::delegation::DelegatedAsk>,
    pub library: Vec<superfik_core::library::LibraryItem>,
    pub policy: superfik_core::policy::Policy,
    pub spent_usd: f64,
    pub usage: Vec<superfik_protocol::MemberUsage>,
    pub teams: Vec<superfik_core::sharing::Team>,
    pub team_open: Option<superfik_core::TeamId>,
}

impl OrganizationState {
    pub fn joined(&self) -> bool {
        self.me.is_some()
    }

    pub fn members_listed(&mut self, mut members: Vec<Colleague>) {
        members.sort_by_key(|member| member.name.to_lowercase());
        self.members = members;
    }
}

impl Session {
    pub fn manages_organization(&self) -> bool {
        let manager = |role: OrgRole| matches!(role, OrgRole::Owner | OrgRole::Admin);
        let Some(me) = self.organization.me else { return false };
        self.organization.members.iter().any(|member| member.user == me.user && manager(member.role))
    }

    pub(super) fn organization_action(&mut self, action: OrgAction) -> Vec<Effect> {
        match action {
            OrgAction::AnswerDelegated(index, allow) => self.answer_delegated(index, allow),
            OrgAction::RemoveFromLibrary(index) => self.remove_from_library(index),
        }
    }

    pub(super) fn refresh_members(&self) -> Vec<Effect> {
        if self.organization.joined() {
            let usage = self.manages_organization().then_some(ClientCommand::ListUsage);
            [ClientCommand::ListMembers, ClientCommand::ListDelegated, ClientCommand::ListTeams]
                .into_iter()
                .chain(usage)
                .map(|command| Effect::Command(Box::new(command)))
                .collect()
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use superfik_core::{OrgId, Principal, UserId};
    use superfik_protocol::{PROTOCOL_VERSION, ServerEvent};

    use super::*;
    use crate::state::tests::commands;
    use crate::state::{Action, SettingsPane};

    fn colleague(name: &str, role: OrgRole) -> Colleague {
        Colleague { user: UserId::new(), name: name.into(), role, pending: false }
    }

    fn joined() -> (Session, Member) {
        let mut session = Session::default();
        let me = Member { user: UserId::new(), org: OrgId::new() };
        let welcome = ServerEvent::Welcome { version: PROTOCOL_VERSION, principal: Principal::Member(me) };
        session.apply(Action::Server(welcome), 0.0);
        (session, me)
    }

    #[test]
    fn members_are_listed_by_name() {
        let mut session = Session::default();
        let members = vec![colleague("bia", OrgRole::Member), colleague("Ana", OrgRole::Member)];
        session.apply(Action::Server(ServerEvent::Members(members)), 0.0);
        let names: Vec<&str> = session.organization.members.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["Ana", "bia"]);
    }

    #[test]
    fn the_server_pane_asks_for_members_only_once_joined() {
        let mut alone = Session::default();
        let effects = alone.apply(Action::ShowSettingsPane(SettingsPane::Server), 0.0);
        assert!(!commands(&effects).iter().any(|c| matches!(c, ClientCommand::ListMembers)));
        let (mut session, _) = joined();
        let effects = session.apply(Action::ShowSettingsPane(SettingsPane::Server), 0.0);
        assert!(commands(&effects).iter().any(|c| matches!(c, ClientCommand::ListMembers)));
    }

    #[test]
    fn the_servers_admins_manage_its_governance() {
        let (mut session, me) = joined();
        assert!(!session.manages_organization());
        let admin = Colleague { user: me.user, ..colleague("Olga", OrgRole::Admin) };
        session.apply(Action::Server(ServerEvent::Members(vec![admin])), 0.0);
        assert!(session.manages_organization());
    }
}
