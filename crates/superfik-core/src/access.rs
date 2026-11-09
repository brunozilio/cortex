//! What a member may do: roles in the organization and in a project, and
//! the ability each command needs. Pure; the server resolves the roles.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrgRole {
    Owner,
    Admin,
    Member,
    Guest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectRole {
    Maintainer,
    Contributor,
    Viewer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ability {
    Read,
    Run,
    Configure,
    Administer,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Colleague {
    pub user: crate::UserId,
    pub name: String,
    pub role: OrgRole,
    #[serde(default)]
    pub pending: bool,
}

pub fn administers(role: OrgRole) -> bool {
    matches!(role, OrgRole::Owner | OrgRole::Admin)
}

pub fn allows(org: OrgRole, project: Option<ProjectRole>, need: Ability) -> bool {
    let project = match (org, project) {
        (OrgRole::Owner | OrgRole::Admin, _) => return true,
        (_, Some(role)) => role,
        (OrgRole::Member, None) => ProjectRole::Contributor,
        (OrgRole::Guest, None) => return false,
    };
    match need {
        Ability::Read => true,
        Ability::Run => project != ProjectRole::Viewer,
        Ability::Configure => project == ProjectRole::Maintainer,
        Ability::Administer => false,
    }
}

#[cfg(test)]
mod tests {
    use super::Ability::*;
    use super::*;

    #[test]
    fn only_owners_and_admins_administer() {
        assert!(administers(OrgRole::Owner) && administers(OrgRole::Admin));
        assert!(!administers(OrgRole::Member) && !administers(OrgRole::Guest));
    }

    #[test]
    fn owners_and_admins_may_do_everything() {
        for role in [OrgRole::Owner, OrgRole::Admin] {
            for need in [Read, Run, Configure, Administer] {
                assert!(allows(role, Some(ProjectRole::Viewer), need), "{role:?} {need:?}");
                assert!(allows(role, None, need), "{role:?} {need:?}");
            }
        }
    }

    #[test]
    fn a_member_without_a_project_role_works_as_a_contributor() {
        assert!(allows(OrgRole::Member, None, Read));
        assert!(allows(OrgRole::Member, None, Run));
        assert!(!allows(OrgRole::Member, None, Configure));
        assert!(!allows(OrgRole::Member, None, Administer));
    }

    #[test]
    fn the_project_role_decides_for_members_and_guests() {
        for role in [OrgRole::Member, OrgRole::Guest] {
            assert!(allows(role, Some(ProjectRole::Viewer), Read));
            assert!(!allows(role, Some(ProjectRole::Viewer), Run));
            assert!(allows(role, Some(ProjectRole::Contributor), Run));
            assert!(!allows(role, Some(ProjectRole::Contributor), Configure));
            assert!(allows(role, Some(ProjectRole::Maintainer), Configure));
            assert!(!allows(role, Some(ProjectRole::Maintainer), Administer));
        }
    }

    #[test]
    fn a_guest_without_a_project_role_may_do_nothing() {
        for need in [Read, Run, Configure, Administer] {
            assert!(!allows(OrgRole::Guest, None, need), "{need:?}");
        }
    }
}
