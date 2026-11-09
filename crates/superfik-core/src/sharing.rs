//! Who reaches what: a session's visibility and a memory's scope. Pure;
//! these rules come on top of the project roles in `access`.

use serde::{Deserialize, Serialize};

use crate::access::{Ability, OrgRole, ProjectRole, allows};
use crate::ids::{TeamId, UserId};

#[derive(Debug, Clone, Copy)]
pub struct Viewer<'a> {
    pub user: UserId,
    pub teams: &'a [TeamId],
    pub org: OrgRole,
    pub project: Option<ProjectRole>,
}

impl Viewer<'_> {
    fn has(&self, need: Ability) -> bool {
        allows(self.org, self.project, need)
    }

    fn in_team(&self, team: TeamId) -> bool {
        self.teams.contains(&team)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Audience {
    Member(UserId),
    Team(TeamId),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    #[default]
    Private,
    Shared(Vec<Audience>),
    Project,
}

impl Visibility {
    pub fn shows(&self, owner: UserId, viewer: &Viewer<'_>) -> bool {
        if viewer.user == owner {
            return true;
        }
        viewer.has(Ability::Read)
            && match self {
                Self::Private => false,
                Self::Shared(audience) => audience.iter().any(|to| match *to {
                    Audience::Member(user) => user == viewer.user,
                    Audience::Team(team) => viewer.in_team(team),
                }),
                Self::Project => true,
            }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryScope {
    #[default]
    Personal,
    Project,
    Team(TeamId),
    Organization,
}

impl MemoryScope {
    pub fn reaches(self, author: UserId, viewer: &Viewer<'_>) -> bool {
        match self {
            Self::Personal => viewer.user == author,
            Self::Project | Self::Organization => viewer.has(Ability::Read),
            Self::Team(team) => viewer.in_team(team) && viewer.has(Ability::Read),
        }
    }

    pub fn writable(self, author: UserId, viewer: &Viewer<'_>) -> bool {
        match self {
            Self::Personal => viewer.user == author,
            Self::Project => viewer.has(Ability::Run),
            Self::Team(team) => viewer.in_team(team) && viewer.has(Ability::Run),
            Self::Organization => viewer.has(Ability::Administer),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Team {
    pub id: TeamId,
    pub name: String,
    pub members: Vec<UserId>,
}

pub fn teams_of(teams: &[Team], user: UserId) -> Vec<TeamId> {
    teams.iter().filter(|team| team.members.contains(&user)).map(|team| team.id).collect()
}

pub fn team_named<'a>(teams: &'a [Team], name: &str) -> Option<&'a Team> {
    teams.iter().find(|team| team.name.eq_ignore_ascii_case(name.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member<'a>(user: UserId, teams: &'a [TeamId]) -> Viewer<'a> {
        Viewer { user, teams, org: OrgRole::Member, project: None }
    }

    #[test]
    fn a_private_session_shows_only_to_its_owner() {
        let (owner, other) = (UserId::new(), UserId::new());
        assert!(Visibility::Private.shows(owner, &member(owner, &[])));
        assert!(!Visibility::Private.shows(owner, &member(other, &[])));
    }

    #[test]
    fn a_shared_session_shows_to_the_people_and_teams_named() {
        let (owner, named, teammate, stranger) = (UserId::new(), UserId::new(), UserId::new(), UserId::new());
        let team = TeamId::new();
        let shared = Visibility::Shared(vec![Audience::Member(named), Audience::Team(team)]);
        assert!(shared.shows(owner, &member(named, &[])));
        assert!(shared.shows(owner, &member(teammate, &[team])));
        assert!(!shared.shows(owner, &member(stranger, &[TeamId::new()])));
    }

    #[test]
    fn a_project_session_shows_to_whoever_reads_the_project() {
        let owner = UserId::new();
        let guest = Viewer { user: UserId::new(), teams: &[], org: OrgRole::Guest, project: None };
        let viewer = Viewer { project: Some(ProjectRole::Viewer), ..guest };
        assert!(Visibility::Project.shows(owner, &member(UserId::new(), &[])));
        assert!(Visibility::Project.shows(owner, &viewer));
        assert!(!Visibility::Project.shows(owner, &guest));
    }

    #[test]
    fn sharing_never_reaches_past_the_project_role() {
        let (owner, guest) = (UserId::new(), UserId::new());
        let shared = Visibility::Shared(vec![Audience::Member(guest)]);
        let outsider = Viewer { user: guest, teams: &[], org: OrgRole::Guest, project: None };
        assert!(!shared.shows(owner, &outsider));
    }

    #[test]
    fn a_personal_memory_reaches_and_is_written_by_its_author_only() {
        let (author, other) = (UserId::new(), UserId::new());
        assert!(MemoryScope::Personal.reaches(author, &member(author, &[])));
        assert!(MemoryScope::Personal.writable(author, &member(author, &[])));
        assert!(!MemoryScope::Personal.reaches(author, &member(other, &[])));
        assert!(!MemoryScope::Personal.writable(author, &member(other, &[])));
    }

    #[test]
    fn a_project_memory_is_read_by_readers_and_written_by_contributors() {
        let author = UserId::new();
        let viewer = Viewer { project: Some(ProjectRole::Viewer), ..member(UserId::new(), &[]) };
        assert!(MemoryScope::Project.reaches(author, &viewer));
        assert!(!MemoryScope::Project.writable(author, &viewer));
        assert!(MemoryScope::Project.writable(author, &member(UserId::new(), &[])));
    }

    #[test]
    fn a_team_memory_stays_inside_the_team() {
        let (author, team) = (UserId::new(), TeamId::new());
        let scope = MemoryScope::Team(team);
        assert!(scope.reaches(author, &member(UserId::new(), &[team])));
        assert!(scope.writable(author, &member(UserId::new(), &[team])));
        assert!(!scope.reaches(author, &member(UserId::new(), &[])));
        assert!(!scope.writable(author, &member(UserId::new(), &[])));
    }

    #[test]
    fn the_organization_memory_reaches_everyone_and_is_kept_by_admins() {
        let author = UserId::new();
        let admin = Viewer { org: OrgRole::Admin, ..member(UserId::new(), &[]) };
        assert!(MemoryScope::Organization.reaches(author, &member(UserId::new(), &[])));
        assert!(!MemoryScope::Organization.writable(author, &member(UserId::new(), &[])));
        assert!(MemoryScope::Organization.writable(author, &admin));
    }

    #[test]
    fn a_member_belongs_to_the_teams_that_list_them_and_teams_are_found_by_name() {
        let (ana, bia) = (UserId::new(), UserId::new());
        let design = Team { id: TeamId::new(), name: "Design".into(), members: vec![ana] };
        let platform = Team { id: TeamId::new(), name: "Platform".into(), members: vec![ana, bia] };
        let teams = [design.clone(), platform.clone()];
        assert_eq!(teams_of(&teams, ana), [design.id, platform.id]);
        assert_eq!(teams_of(&teams, bia), [platform.id]);
        assert_eq!(team_named(&teams, " design ").map(|t| t.id), Some(design.id));
        assert!(team_named(&teams, "sales").is_none());
    }
}
