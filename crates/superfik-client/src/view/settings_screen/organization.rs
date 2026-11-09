//! The Server pane's team rows: the asks waiting for an approver, the
//! users as the server lists them (for members; owners and admins manage
//! them in `server::administration`) and the library. Pure.

use superfik_core::access::OrgRole;
use superfik_i18n::{Strings, Text};

use super::collaboration;
use super::rows::{Control, Group, Line};
use crate::state::OrganizationState;

pub fn groups(organization: &OrganizationState, manages: bool, strings: &Strings) -> Vec<Group> {
    let t = |text: Text<'_>| strings.t(text);
    if organization.me.is_none() {
        return Vec::new();
    }
    let mut groups: Vec<Group> = collaboration::approvals(organization, strings).into_iter().collect();
    if !manages {
        let lines = organization
            .members
            .iter()
            .map(|member| {
                let role = t(match member.role {
                    OrgRole::Owner => Text::RoleOwner,
                    OrgRole::Admin => Text::RoleAdmin,
                    OrgRole::Member => Text::RoleMember,
                    OrgRole::Guest => Text::RoleGuest,
                });
                Line { browser: None, title: member.name.clone(), detail: Some(role), control: Control::None }
            })
            .collect();
        groups.push(Group { heading: Some(t(Text::OrganizationMembers)), lines });
    }
    groups.extend(collaboration::library(organization, manages, strings));
    groups
}

#[cfg(test)]
mod tests {
    use superfik_core::access::Colleague;
    use superfik_core::{Member, OrgId, UserId};
    use superfik_i18n::Language;

    use super::*;

    fn colleague(user: UserId, name: &str, role: OrgRole) -> Colleague {
        Colleague { user, name: name.into(), role, pending: false }
    }

    #[test]
    fn alone_there_is_no_team_to_show() {
        let english = Strings::new(Language::English);
        assert!(groups(&OrganizationState::default(), false, &english).is_empty());
    }

    #[test]
    fn members_see_who_belongs_with_their_role_and_change_nothing() {
        let (me, ana) = (UserId::new(), UserId::new());
        let organization = OrganizationState {
            me: Some(Member { user: me, org: OrgId::new() }),
            members: vec![colleague(ana, "ana", OrgRole::Admin), colleague(me, "olga", OrgRole::Member)],
            ..OrganizationState::default()
        };
        let shown = groups(&organization, false, &Strings::new(Language::Portuguese));
        let members = &shown[0].lines;
        assert_eq!(members[0].detail.as_deref(), Some("Administrador"));
        assert!(members.iter().all(|line| line.control == Control::None));
        let managed = groups(&organization, true, &Strings::new(Language::Portuguese));
        assert!(managed.iter().all(|group| group.heading.as_deref() != Some("Membros")), "managed elsewhere");
    }
}
