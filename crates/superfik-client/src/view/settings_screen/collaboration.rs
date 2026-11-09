//! The Organization pane's collaboration groups: asks waiting for this
//! approver, and the organization's library. Pure.

use superfik_core::library::LibraryKind;
use superfik_i18n::{Strings, Text};
use superfik_ui::icons;

use super::rows::{Control, Group, Line, Run};
use crate::state::{OrganizationState, Setting, TeamChoice};

pub fn approvals(organization: &OrganizationState, strings: &Strings) -> Option<Group> {
    let t = |text: Text<'_>| strings.t(text);
    let lines: Vec<Line> = organization
        .delegated
        .iter()
        .enumerate()
        .map(|(index, waiting)| {
            let (tool, subject) = match &waiting.ask.kind {
                superfik_core::AskKind::Permission { tool, subject, .. } => (tool.as_str(), subject.as_str()),
                superfik_core::AskKind::Questions(_) => ("", ""),
            };
            let detail = match &waiting.session {
                Some(session) => format!("{subject} · {session}"),
                None => subject.to_owned(),
            };
            Line {
                browser: None,
                title: format!("{} · {tool}", waiting.requester_name),
                detail: Some(detail),
                control: Control::Buttons(vec![
                    (t(Text::AskDeny), icons::CLOSE, Run::AnswerDelegated(index, false)),
                    (t(Text::AskAllow), icons::CHECK, Run::AnswerDelegated(index, true)),
                ]),
            }
        })
        .collect();
    (!lines.is_empty()).then(|| Group { heading: Some(t(Text::WaitingForYourApproval)), lines })
}

pub fn library(organization: &OrganizationState, may_remove: bool, strings: &Strings) -> Option<Group> {
    let lines: Vec<Line> = organization
        .library
        .iter()
        .enumerate()
        .map(|(index, item)| Line {
            browser: None,
            title: item.name.clone(),
            detail: Some(strings.t(match item.kind {
                LibraryKind::Skill => Text::LibrarySkillBy(&item.author_name),
                LibraryKind::Agent => Text::LibraryAgentBy(&item.author_name),
            })),
            control: if may_remove {
                let remove = strings.t(Text::RemoveMember);
                Control::Buttons(vec![(remove, icons::DELETE, Run::RemoveFromLibrary(index))])
            } else {
                Control::None
            },
        })
        .collect();
    (!lines.is_empty()).then(|| Group { heading: Some(strings.t(Text::OrganizationLibrary)), lines })
}

pub fn teams(organization: &OrganizationState, may_edit: bool, strings: &Strings) -> Vec<Group> {
    let t = |text: Text<'_>| strings.t(text);
    let mut lines: Vec<Line> = organization
        .teams
        .iter()
        .map(|team| {
            let open = organization.team_open == Some(team.id);
            let control = if may_edit {
                let (label, toggle) = if open {
                    (t(Text::TeamDone), TeamChoice::Open(None))
                } else {
                    (t(Text::TeamEdit), TeamChoice::Open(Some(team.id)))
                };
                Control::Buttons(vec![
                    (t(Text::RemoveMember), icons::DELETE, Run::Team(TeamChoice::Remove(team.id))),
                    (label, icons::PERSON, Run::Team(toggle)),
                ])
            } else {
                Control::None
            };
            Line {
                browser: None,
                title: team.name.clone(),
                detail: Some(t(Text::TeamMembers(team.members.len()))),
                control,
            }
        })
        .collect();
    if may_edit {
        lines.push(Line { browser: None, title: t(Text::TeamHint), detail: None, control: Control::None });
    }
    if lines.is_empty() {
        return Vec::new();
    }
    let mut groups = vec![Group { heading: Some(t(Text::OrganizationTeams)), lines }];
    let open = organization.team_open.and_then(|id| organization.teams.iter().find(|team| team.id == id));
    if let (true, Some(team)) = (may_edit, open) {
        let lines = organization
            .members
            .iter()
            .map(|member| {
                let on = team.members.contains(&member.user);
                let flip = Setting::Team(TeamChoice::Member { team: team.id, user: member.user, member: !on });
                Line { browser: None, title: member.name.clone(), detail: None, control: Control::Switch { on, flip } }
            })
            .collect();
        groups.push(Group { heading: Some(t(Text::TeamWho(&team.name))), lines });
    }
    groups
}

#[cfg(test)]
mod tests {
    use superfik_core::library::LibraryItem;
    use superfik_core::{AgentAsk, AskId, AskKind, RunId, UserId};
    use superfik_i18n::Language;

    use super::*;

    #[test]
    fn asks_waiting_for_an_approver_are_allowed_or_denied() {
        let kind = AskKind::Permission { tool: "Bash".into(), subject: "cargo publish".into(), rememberable: false };
        let waiting = superfik_core::delegation::DelegatedAsk {
            run: RunId::new(),
            ask: AgentAsk { id: AskId("p1".into()), kind },
            requester: UserId::new(),
            requester_name: "Ana".into(),
            session: Some("Release".into()),
        };
        let organization = OrganizationState { delegated: vec![waiting], ..OrganizationState::default() };
        let group = approvals(&organization, &Strings::new(Language::English)).expect("waiting");
        assert_eq!(group.heading.as_deref(), Some("Waiting for your approval"));
        let line = &group.lines[0];
        assert_eq!((line.title.as_str(), line.detail.as_deref()), ("Ana · Bash", Some("cargo publish · Release")));
        let Control::Buttons(buttons) = &line.control else { panic!("no buttons") };
        let runs: Vec<Run> = buttons.iter().map(|(.., run)| *run).collect();
        assert_eq!(runs, [Run::AnswerDelegated(0, false), Run::AnswerDelegated(0, true)]);
    }

    #[test]
    fn the_library_names_who_published_and_only_admins_remove() {
        let item = LibraryItem {
            kind: LibraryKind::Agent,
            name: "reviewer".into(),
            body: String::new(),
            author_name: "Olga".into(),
        };
        let organization = OrganizationState { library: vec![item], ..OrganizationState::default() };
        let portuguese = Strings::new(Language::Portuguese);
        let group = library(&organization, true, &portuguese).expect("listed");
        assert_eq!(group.lines[0].detail.as_deref(), Some("Agente · publicado por Olga"));
        assert!(matches!(&group.lines[0].control, Control::Buttons(b) if b[0].2 == Run::RemoveFromLibrary(0)));
        let group = library(&organization, false, &portuguese).expect("listed");
        assert_eq!(group.lines[0].control, Control::None);
        assert!(library(&OrganizationState::default(), true, &portuguese).is_none());
    }

    #[test]
    fn an_open_team_lists_every_member_with_a_switch() {
        use superfik_core::access::{Colleague, OrgRole};
        let (ana, bia) = (UserId::new(), UserId::new());
        let colleague = |user, name: &str| Colleague { user, name: name.into(), role: OrgRole::Member, pending: false };
        let design = superfik_core::sharing::Team {
            id: superfik_core::TeamId::new(),
            name: "Design".into(),
            members: vec![ana],
        };
        let mut organization = OrganizationState {
            members: vec![colleague(ana, "Ana"), colleague(bia, "Bia")],
            teams: vec![design.clone()],
            ..OrganizationState::default()
        };
        let english = Strings::new(Language::English);
        let groups = teams(&organization, true, &english);
        assert_eq!((groups.len(), groups[0].lines[0].detail.as_deref()), (1, Some("1 member")));
        organization.team_open = Some(design.id);
        let groups = teams(&organization, true, &english);
        assert_eq!(groups[1].heading.as_deref(), Some("Who is in Design"));
        let switches: Vec<bool> =
            groups[1].lines.iter().map(|line| matches!(line.control, Control::Switch { on: true, .. })).collect();
        assert_eq!(switches, [true, false]);
        assert!(teams(&OrganizationState::default(), false, &english).is_empty());
    }
}
