//! Teams in the app: `/share <team>` shares the active session with one,
//! `/team <name>` makes one (owners and admins), and Settings ›
//! Organization says who is in each (`Setting::Team`).

use superfik_core::sharing::{self, Audience, Visibility};
use superfik_core::{TeamId, UserId, command};
use superfik_protocol::ClientCommand;

use super::{Effect, Notice, Session};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TeamChoice {
    Open(Option<TeamId>),
    Member { team: TeamId, user: UserId, member: bool },
    Remove(TeamId),
}

fn send(command: ClientCommand) -> Vec<Effect> {
    vec![Effect::Command(Box::new(command))]
}

impl Session {
    pub(super) fn run_team_command(&mut self) -> Option<Vec<Effect>> {
        let text = self.conversation().composer.text().to_owned();
        if let Some(name) = command::argument(&text, "share") {
            self.conversation_mut().composer.take();
            return Some(self.share_with_team(name));
        }
        let name = command::argument(&text, "team")?.to_owned();
        self.conversation_mut().composer.take();
        if !self.manages_organization() {
            self.notice = Some(Notice::TeamsAdminsOnly);
            return Some(Vec::new());
        }
        Some(send(ClientCommand::CreateTeam { name }))
    }

    fn share_with_team(&mut self, name: &str) -> Vec<Effect> {
        let Some(team) = sharing::team_named(&self.organization.teams, name).cloned() else {
            self.notice = Some(Notice::NoSuchTeam(name.to_owned()));
            return Vec::new();
        };
        let conversation = self.conversation_mut();
        if conversation.shared_by.is_some() {
            return Vec::new();
        }
        conversation.visibility = Visibility::Shared(vec![Audience::Team(team.id)]);
        self.notice = Some(Notice::SharedWithTeam(team.name));
        Vec::new()
    }

    pub(super) fn choose_team(&mut self, choice: TeamChoice) -> Vec<Effect> {
        if !self.manages_organization() {
            return Vec::new();
        }
        match choice {
            TeamChoice::Open(team) => {
                self.organization.team_open = team;
                Vec::new()
            }
            TeamChoice::Member { team, user, member } => send(ClientCommand::SetTeamMember { team, user, member }),
            TeamChoice::Remove(team) => {
                if self.organization.team_open == Some(team) {
                    self.organization.team_open = None;
                }
                send(ClientCommand::RemoveTeam { team })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use superfik_core::access::{Colleague, OrgRole};
    use superfik_core::sharing::Team;
    use superfik_core::{Member, OrgId, Principal};
    use superfik_protocol::{PROTOCOL_VERSION, ServerEvent};

    use super::*;
    use crate::state::tests::commands;
    use crate::state::{Action, EditKey, Setting};

    fn member(role: OrgRole) -> (Session, Member) {
        let mut session = Session::new(Some("/repo".into()));
        let me = Member { user: UserId::new(), org: OrgId::new() };
        let welcome = ServerEvent::Welcome { version: PROTOCOL_VERSION, principal: Principal::Member(me) };
        session.apply(Action::Server(welcome), 0.0);
        let colleague = Colleague { user: me.user, name: "olga".into(), role, pending: false };
        session.apply(Action::Server(ServerEvent::Members(vec![colleague])), 0.0);
        (session, me)
    }

    fn typed(session: &mut Session, text: &str) -> Vec<Effect> {
        session.apply(Action::Edit(EditKey::Insert(text.into())), 0.0);
        session.apply(Action::Submit, 0.0)
    }

    #[test]
    fn a_session_is_shared_with_a_team_by_name() {
        let (mut session, _) = member(OrgRole::Member);
        let design = Team { id: TeamId::new(), name: "Design".into(), members: vec![] };
        session.apply(Action::Server(ServerEvent::Teams(vec![design.clone()])), 0.0);
        typed(&mut session, "/share design");
        assert_eq!(session.conversation().visibility, Visibility::Shared(vec![Audience::Team(design.id)]));
        assert_eq!(session.notice, Some(Notice::SharedWithTeam("Design".into())));
        typed(&mut session, "/share sales");
        assert_eq!(session.notice, Some(Notice::NoSuchTeam("sales".into())));
        assert!(session.conversation().turns.is_empty(), "nothing went to the model");
    }

    #[test]
    fn admins_make_teams_and_say_who_is_in_them() {
        let (mut session, me) = member(OrgRole::Admin);
        let effects = typed(&mut session, "/team Design Ops");
        assert!(matches!(commands(&effects)[..], [ClientCommand::CreateTeam { name }] if name == "Design Ops"));
        let team = TeamId::new();
        let choose = |choice| Action::ChooseSetting(Setting::Team(choice));
        session.apply(choose(TeamChoice::Open(Some(team))), 0.0);
        assert_eq!(session.organization.team_open, Some(team));
        let effects = session.apply(choose(TeamChoice::Member { team, user: me.user, member: true }), 0.0);
        assert!(matches!(commands(&effects)[..], [ClientCommand::SetTeamMember { member: true, .. }]));
        let (mut plain, _) = member(OrgRole::Member);
        typed(&mut plain, "/team Design");
        assert_eq!(plain.notice, Some(Notice::TeamsAdminsOnly));
    }
}
