//! Settings › Server's users, for whoever runs the server: which rows
//! offer what, and what a row's buttons send. Rows name users by id, not
//! position: the list is sorted again whenever the server sends it.

use superfik_core::UserId;
use superfik_core::access::{Colleague, OrgRole};
use superfik_protocol::ClientCommand;

use super::command;
use crate::state::{Effect, Session};

impl Session {
    pub fn manages_user(&self, user: UserId) -> bool {
        self.other_user(user).is_some_and(|listed| listed.role != OrgRole::Owner)
    }

    pub fn offers_new_code(&self, user: UserId) -> bool {
        let mine = self.organization.me.and_then(|me| self.listed_user(me.user)).map(|me| me.role);
        self.other_user(user).is_some_and(|listed| listed.role != OrgRole::Owner || mine == Some(OrgRole::Owner))
    }

    pub(in crate::state) fn members_arrived(&mut self, members: Vec<Colleague>) -> Vec<Effect> {
        let administered = self.administers_server();
        self.organization.members_listed(members);
        if administered || !self.server_pane_open() { Vec::new() } else { self.server_pane_shown() }
    }

    pub(super) fn new_code(&self, user: UserId) -> Vec<Effect> {
        if !self.offers_new_code(user) {
            return Vec::new();
        }
        command(ClientCommand::NewFirstAccessCode { user })
    }

    pub(super) fn toggle_admin(&self, user: UserId) -> Vec<Effect> {
        if !self.manages_user(user) {
            return Vec::new();
        }
        let admin = self.listed_user(user).is_some_and(|listed| listed.role == OrgRole::Admin);
        let role = if admin { OrgRole::Member } else { OrgRole::Admin };
        command(ClientCommand::SetUserRole { user, role })
    }

    pub(super) fn remove_user(&self, user: UserId) -> Vec<Effect> {
        if !self.manages_user(user) {
            return Vec::new();
        }
        command(ClientCommand::RemoveUser { user })
    }

    fn listed_user(&self, user: UserId) -> Option<&Colleague> {
        self.organization.members.iter().find(|member| member.user == user)
    }

    fn other_user(&self, user: UserId) -> Option<&Colleague> {
        let me = self.organization.me.map(|me| me.user);
        self.listed_user(user).filter(|listed| self.administers_server() && Some(listed.user) != me)
    }
}
