//! Settings › Server's own rows (ADR 0017): alone, the form that connects
//! to a team's server; connected, who the app is signed in as and, for
//! owners and admins, the server's settings and users.

use superfik_core::access::OrgRole;
use superfik_i18n::{Strings, Text};
use superfik_ui::icons;

use super::rows::{Control, Field, Group, Line, Run, ServerRun};
use crate::state::{AddressProblem, ServerField, Session};

fn field(session: &Session, field: ServerField) -> Control {
    let link = &session.server_link;
    Control::Field { edit: link.edit(field).clone(), focused: link.focus == Some(field), field: Field::Server(field) }
}

fn line(title: String, detail: Option<String>, control: Control) -> Line {
    Line { browser: None, title, detail, control }
}

fn role_name(role: OrgRole) -> Text<'static> {
    match role {
        OrgRole::Owner => Text::RoleOwner,
        OrgRole::Admin => Text::RoleAdmin,
        OrgRole::Member => Text::RoleMember,
        OrgRole::Guest => Text::RoleGuest,
    }
}

pub fn connection(session: &Session, strings: &Strings) -> Vec<Group> {
    let t = |text: Text<'_>| strings.t(text);
    let link = &session.server_link;
    if session.remote {
        let title = link.joined.as_ref().map_or_else(
            || t(Text::SettingsServer),
            |joined| t(Text::ServerConnectedAs(&joined.name, &joined.address)),
        );
        let me = session.organization.me.and_then(|me| session.organization.members.iter().find(|m| m.user == me.user));
        let detail = me.map(|me| t(role_name(me.role)));
        let disconnect =
            Control::Buttons(vec![(t(Text::ServerDisconnect), icons::PERSON, Run::Server(ServerRun::Leave))]);
        return vec![Group { heading: None, lines: vec![line(title, detail, disconnect)] }];
    }
    let alone = line(t(Text::ServerLocal), Some(t(Text::ServerLocalCaption)), Control::None);
    let mut lines = vec![
        line(t(Text::ServerAddress), Some(t(Text::ServerAddressCaption)), field(session, ServerField::Address)),
        line(t(Text::ServerUsername), None, field(session, ServerField::Name)),
    ];
    if link.first_access {
        lines.push(line(t(Text::ServerCode), Some(t(Text::ServerCodeCaption)), field(session, ServerField::Code)));
        lines.push(line(
            t(Text::ServerNewPassword),
            Some(t(Text::ServerNewPasswordCaption)),
            field(session, ServerField::Password),
        ));
    } else {
        lines.push(line(t(Text::ServerPassword), None, field(session, ServerField::Password)));
    }
    let toggle = if link.first_access { Text::ServerHavePassword } else { Text::ServerFirstAccess };
    let connect = if link.joining { Text::ServerConnecting } else { Text::ServerConnect };
    let mut buttons = vec![
        (t(connect), icons::PLUG, Run::Server(ServerRun::Join)),
        (t(toggle), icons::PERSON, Run::Server(ServerRun::ToggleFirstAccess)),
    ];
    if link.problem == Some(AddressProblem::Insecure) {
        buttons.insert(1, (t(Text::ServerAllowInsecure), icons::SHIELD, Run::Server(ServerRun::AllowInsecure)));
    }
    lines.push(line(String::new(), said(session, strings), Control::Buttons(buttons)));
    vec![Group { heading: None, lines: vec![alone] }, Group { heading: Some(t(Text::ServerConnectHeading)), lines }]
}

fn said(session: &Session, strings: &Strings) -> Option<String> {
    let link = &session.server_link;
    match link.problem {
        Some(AddressProblem::Invalid) => return Some(strings.t(Text::ServerAddressInvalid)),
        Some(AddressProblem::Insecure) => return Some(strings.t(Text::ServerInsecure)),
        None => {}
    }
    let hint = link.http_hint.then(|| strings.t(Text::ServerTryHttp));
    match (link.said.clone(), hint) {
        (Some(said), Some(hint)) => Some(format!("{said} {hint}")),
        (said, hint) => said.or(hint),
    }
}

pub fn administration(session: &Session, strings: &Strings) -> Vec<Group> {
    if !session.administers_server() {
        return Vec::new();
    }
    let t = |text: Text<'_>| strings.t(text);
    let link = &session.server_link;
    let mut groups = Vec::new();
    if let Some((name, code)) = &link.code_shown {
        let buttons = Control::Buttons(vec![
            (t(Text::ServerCopy), icons::LAYERS, Run::Server(ServerRun::CopyCode)),
            (t(Text::ServerDone), icons::CHECK, Run::Server(ServerRun::DismissCode)),
        ]);
        let title = format!("{} · {code}", t(Text::ServerCodeFor(name)));
        groups.push(Group { heading: None, lines: vec![line(title, Some(t(Text::ServerCodeShownCaption)), buttons)] });
    }
    groups.push(Group {
        heading: Some(t(Text::ServerHeading)),
        lines: vec![
            line(t(Text::ServerDomain), Some(t(Text::ServerDomainCaption)), field(session, ServerField::Domain)),
            line(
                String::new(),
                None,
                Control::Buttons(vec![(t(Text::ServerSave), icons::CHECK, Run::Server(ServerRun::SaveSettings))]),
            ),
        ],
    });
    let users = session
        .organization
        .members
        .iter()
        .map(|member| {
            let role = t(role_name(member.role));
            let detail = if member.pending { format!("{role} · {}", t(Text::ServerPending)) } else { role };
            let mut buttons = Vec::new();
            if session.offers_new_code(member.user) {
                buttons.push((t(Text::ServerNewCode), icons::PERSON, Run::Server(ServerRun::NewCode(member.user))));
            }
            if session.manages_user(member.user) {
                let toggle = if member.role == OrgRole::Admin { Text::ServerMakeMember } else { Text::ServerMakeAdmin };
                buttons.push((t(toggle), icons::SHIELD, Run::Server(ServerRun::ToggleAdmin(member.user))));
                buttons.push((t(Text::RemoveMember), icons::DELETE, Run::Server(ServerRun::Remove(member.user))));
            }
            let control = if buttons.is_empty() { Control::None } else { Control::Buttons(buttons) };
            line(member.name.clone(), Some(detail), control)
        })
        .collect();
    groups.push(Group { heading: Some(t(Text::ServerUsers)), lines: users });
    let role = link.new_role.unwrap_or(OrgRole::Member);
    let roles = [OrgRole::Member, OrgRole::Admin, OrgRole::Guest];
    groups.push(Group {
        heading: Some(t(Text::ServerAddUser)),
        lines: vec![
            line(t(Text::ServerUsername), Some(t(Text::ServerAddUserCaption)), field(session, ServerField::NewUser)),
            line(
                String::new(),
                None,
                Control::Buttons(
                    roles
                        .into_iter()
                        .map(|option| {
                            let icon = if option == role { icons::CHECK } else { icons::PERSON };
                            (t(role_name(option)), icon, Run::Server(ServerRun::NewUserRole(option)))
                        })
                        .chain([(t(Text::ServerAdd), icons::PLUS, Run::Server(ServerRun::AddUser))])
                        .collect(),
                ),
            ),
        ],
    });
    groups
}

#[cfg(test)]
mod tests {
    use superfik_core::access::Colleague;
    use superfik_core::{Member, OrgId, Principal, UserId};
    use superfik_i18n::Language;
    use superfik_protocol::{PROTOCOL_VERSION, ServerEvent};

    use super::*;
    use crate::state::{Action, ServerAction};

    fn runs(groups: &[Group]) -> Vec<Run> {
        groups
            .iter()
            .flat_map(|group| &group.lines)
            .flat_map(|line| match &line.control {
                Control::Buttons(buttons) => buttons.iter().map(|(.., run)| *run).collect(),
                _ => Vec::new(),
            })
            .collect()
    }

    fn fields(groups: &[Group]) -> Vec<Field> {
        let lines = groups.iter().flat_map(|group| &group.lines);
        lines
            .filter_map(|line| if let Control::Field { field, .. } = line.control { Some(field) } else { None })
            .collect()
    }

    #[test]
    fn alone_it_offers_the_form_and_the_first_access_adds_the_code() {
        let english = Strings::new(Language::English);
        let mut session = Session::default();
        let shown = connection(&session, &english);
        let server = |field| Field::Server(field);
        assert_eq!(fields(&shown), [ServerField::Address, ServerField::Name, ServerField::Password].map(server));
        assert!(runs(&shown).contains(&Run::Server(ServerRun::Join)));
        session.apply(Action::ServerLink(ServerAction::ToggleFirstAccess), 0.0);
        let first = connection(&session, &english);
        assert!(fields(&first).contains(&Field::Server(ServerField::Code)));
        assert!(administration(&session, &english).is_empty(), "nothing to run alone");
    }

    #[test]
    fn plain_http_to_another_computer_is_explained_and_offered_only_when_asked() {
        let english = Strings::new(Language::English);
        let mut session = Session::default();
        assert!(!runs(&connection(&session, &english)).contains(&Run::Server(ServerRun::AllowInsecure)));
        for (field, text) in [
            (ServerField::Address, "http://192.168.1.5:7420"),
            (ServerField::Name, "ana"),
            (ServerField::Password, "pw"),
        ] {
            session.apply(Action::ServerLink(ServerAction::Focus(field)), 0.0);
            session.apply(Action::ServerLink(ServerAction::Edit(crate::state::EditKey::Insert(text.into()))), 0.0);
        }
        session.apply(Action::ServerLink(ServerAction::Join), 0.0);
        let shown = connection(&session, &english);
        assert!(runs(&shown).contains(&Run::Server(ServerRun::AllowInsecure)));
        let warning = english.t(Text::ServerInsecure);
        assert!(shown.iter().flat_map(|g| &g.lines).any(|line| line.detail.as_ref() == Some(&warning)));
    }

    #[test]
    fn owners_run_the_server_and_never_remove_themselves() {
        let mut session = Session::default();
        let me = Member { user: UserId::new(), org: OrgId::new() };
        let welcome = ServerEvent::Welcome { version: PROTOCOL_VERSION, principal: Principal::Member(me) };
        session.apply(Action::Server(welcome), 0.0);
        let ana_id = UserId::new();
        let members = vec![
            Colleague { user: ana_id, name: "ana".into(), role: OrgRole::Member, pending: true },
            Colleague { user: me.user, name: "olga".into(), role: OrgRole::Owner, pending: false },
        ];
        session.apply(Action::Server(ServerEvent::Members(members)), 0.0);
        let portuguese = Strings::new(Language::Portuguese);
        let shown = administration(&session, &portuguese);
        let all = runs(&shown);
        assert!(all.contains(&Run::Server(ServerRun::Remove(ana_id))));
        assert!(all.contains(&Run::Server(ServerRun::NewCode(ana_id))));
        assert!(!all.contains(&Run::Server(ServerRun::Remove(me.user))));
        assert!(!all.contains(&Run::Server(ServerRun::NewCode(me.user))), "not one's own sign-ins");
        let ana = shown.iter().flat_map(|g| &g.lines).find(|line| line.title == "ana").expect("ana");
        assert_eq!(ana.detail.as_deref(), Some("Membro · primeiro acesso pendente"));
        assert!(runs(&connection(&session, &portuguese)).contains(&Run::Server(ServerRun::Leave)));
    }
}
