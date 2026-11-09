use superfik_core::access::{Colleague, OrgRole};
use superfik_core::{Member, OrgId, Principal, UserId};
use superfik_protocol::{PROTOCOL_VERSION, ServerEvent};

use super::*;
use crate::state::{Action, EditKey};

fn commands(effects: &[Effect]) -> Vec<ClientCommand> {
    crate::state::tests::commands(effects).into_iter().cloned().collect()
}

fn act(session: &mut Session, action: ServerAction) -> Vec<Effect> {
    session.apply(Action::ServerLink(action), 0.0)
}

fn write(session: &mut Session, field: ServerField, text: &str) {
    act(session, ServerAction::Focus(field));
    act(session, ServerAction::Edit(EditKey::Insert(text.into())));
}

fn joins(effects: &[Effect]) -> Option<JoinRequest> {
    effects
        .iter()
        .find_map(|effect| if let Effect::JoinServer(request) = effect { Some(request.clone()) } else { None })
}

#[test]
fn signing_in_needs_the_address_the_name_and_the_password() {
    let mut session = Session::default();
    write(&mut session, ServerField::Address, "superfik.acme.io");
    write(&mut session, ServerField::Name, "ana");
    assert_eq!(joins(&act(&mut session, ServerAction::Submit)), None, "no password yet");
    write(&mut session, ServerField::Password, "correct horse");
    let request = joins(&act(&mut session, ServerAction::Submit)).expect("joins");
    let parts = (request.address.as_str(), request.name.as_str(), request.code.as_deref());
    assert_eq!(parts, ("superfik.acme.io", "ana", None));
    assert!(session.server_link.joining);
    assert_eq!(joins(&act(&mut session, ServerAction::Join)), None, "once");
    assert!(!format!("{request:?}").contains("correct horse"), "the password stays out of logs");
}

#[test]
fn the_first_access_also_takes_the_code() {
    let mut session = Session::default();
    act(&mut session, ServerAction::ToggleFirstAccess);
    write(&mut session, ServerField::Address, "superfik.acme.io");
    write(&mut session, ServerField::Name, "ana");
    write(&mut session, ServerField::Password, "correct horse");
    assert_eq!(joins(&act(&mut session, ServerAction::Join)), None, "no code yet");
    write(&mut session, ServerField::Code, "ABCD-EFGH-JKMN");
    let request = joins(&act(&mut session, ServerAction::Join)).expect("joins");
    assert_eq!(request.code.as_deref(), Some("ABCD-EFGH-JKMN"));
    let joined = Joined { address: "superfik.acme.io".into(), name: "ana".into() };
    act(&mut session, ServerAction::Joined(Ok(joined.clone())));
    assert_eq!(session.server_link.joined, Some(joined));
    assert!(session.server_link.password.text().is_empty() && !session.server_link.first_access, "forgotten");
}

#[test]
fn a_refused_sign_in_is_said_and_can_be_tried_again() {
    let mut session = Session::default();
    write(&mut session, ServerField::Address, "superfik.acme.io");
    write(&mut session, ServerField::Name, "ana");
    write(&mut session, ServerField::Password, "wrong password");
    act(&mut session, ServerAction::Join);
    act(&mut session, ServerAction::Joined(Err("wrong username or password".into())));
    assert_eq!(session.server_link.said.as_deref(), Some("wrong username or password"));
    assert!(joins(&act(&mut session, ServerAction::Join)).is_some());
}

#[test]
fn tab_moves_through_the_card_and_wraps() {
    let mut session = Session::default();
    act(&mut session, ServerAction::Focus(ServerField::Address));
    act(&mut session, ServerAction::Step(true));
    act(&mut session, ServerAction::Step(true));
    assert_eq!(session.server_link.focus, Some(ServerField::Password), "no code without the first access");
    act(&mut session, ServerAction::Step(true));
    assert_eq!(session.server_link.focus, Some(ServerField::Address));
    act(&mut session, ServerAction::Step(false));
    assert_eq!(session.server_link.focus, Some(ServerField::Password));
}

fn as_owner() -> (Session, UserId) {
    let mut session = Session::default();
    let me = Member { user: UserId::new(), org: OrgId::new() };
    session.apply(
        Action::Server(ServerEvent::Welcome { version: PROTOCOL_VERSION, principal: Principal::Member(me) }),
        0.0,
    );
    let ana = UserId::new();
    let members = vec![
        Colleague { user: ana, name: "ana".into(), role: OrgRole::Member, pending: true },
        Colleague { user: me.user, name: "olga".into(), role: OrgRole::Owner, pending: false },
    ];
    session.apply(Action::Server(ServerEvent::Members(members)), 0.0);
    (session, ana)
}

#[test]
fn owners_add_users_and_see_their_code_once() {
    let (mut session, _) = as_owner();
    assert!(session.administers_server());
    write(&mut session, ServerField::NewUser, "bia");
    act(&mut session, ServerAction::NewUserRole(OrgRole::Admin));
    let sent = commands(&act(&mut session, ServerAction::Submit));
    assert_eq!(sent, [ClientCommand::AddUser { name: "bia".into(), role: OrgRole::Admin }]);
    assert!(session.server_link.new_user.text().is_empty());
    let code = ServerEvent::FirstAccessCode { user: UserId::new(), name: "bia".into(), code: "ABCD".into() };
    session.apply(Action::Server(code), 0.0);
    let copied = act(&mut session, ServerAction::CopyCode);
    assert!(copied.iter().any(|effect| matches!(effect, Effect::CopyText(code) if code == "ABCD")));
    act(&mut session, ServerAction::DismissCode);
    assert_eq!(session.server_link.code_shown, None);
}

#[test]
fn listed_users_get_new_codes_roles_and_removal() {
    let (mut session, ana) = as_owner();
    assert_eq!(
        commands(&act(&mut session, ServerAction::NewCode(ana))),
        [ClientCommand::NewFirstAccessCode { user: ana }]
    );
    assert_eq!(
        commands(&act(&mut session, ServerAction::ToggleAdmin(ana))),
        [ClientCommand::SetUserRole { user: ana, role: OrgRole::Admin }]
    );
    assert_eq!(commands(&act(&mut session, ServerAction::Remove(ana))), [ClientCommand::RemoveUser { user: ana }]);
    assert!(act(&mut session, ServerAction::Remove(UserId::new())).is_empty(), "no longer listed");
}

#[test]
fn a_user_row_acts_on_its_user_after_the_list_is_sorted_again() {
    let (mut session, ana) = as_owner();
    let mut members = session.organization.members.clone();
    members.push(Colleague { user: UserId::new(), name: "aaron".into(), role: OrgRole::Member, pending: true });
    session.apply(Action::Server(ServerEvent::Members(members)), 0.0);
    assert_eq!(commands(&act(&mut session, ServerAction::Remove(ana))), [ClientCommand::RemoveUser { user: ana }]);
}

#[test]
fn a_new_code_is_never_for_oneself_nor_an_admins_for_an_owner() {
    let (mut session, ana) = as_owner();
    let me = session.organization.me.expect("joined").user;
    assert!(session.offers_new_code(ana));
    assert!(!session.offers_new_code(me), "it would end one's own sign-ins");
    assert!(act(&mut session, ServerAction::NewCode(me)).is_empty());
    let owner = UserId::new();
    let members = vec![
        Colleague { user: ana, name: "ana".into(), role: OrgRole::Member, pending: false },
        Colleague { user: me, name: "olga".into(), role: OrgRole::Admin, pending: false },
        Colleague { user: owner, name: "zoe".into(), role: OrgRole::Owner, pending: false },
    ];
    session.apply(Action::Server(ServerEvent::Members(members)), 0.0);
    assert!(session.offers_new_code(ana));
    assert!(!session.offers_new_code(owner), "the server would refuse");
    assert!(act(&mut session, ServerAction::NewCode(owner)).is_empty());
    assert!(!session.manages_user(owner) && !session.manages_user(me) && session.manages_user(ana));
    assert!(act(&mut session, ServerAction::Remove(owner)).is_empty());
    assert!(act(&mut session, ServerAction::ToggleAdmin(me)).is_empty());
}

#[test]
fn an_admin_on_the_server_pane_is_shown_the_settings_once_the_members_say_so() {
    let mut session = Session::default();
    session.apply(Action::ShowSettingsPane(crate::state::SettingsPane::Server), 0.0);
    session.apply(Action::OpenSettings, 0.0);
    let me = Member { user: UserId::new(), org: OrgId::new() };
    let welcome = ServerEvent::Welcome { version: PROTOCOL_VERSION, principal: Principal::Member(me) };
    let effects = session.apply(Action::Server(welcome), 0.0);
    assert!(commands(&effects).contains(&ClientCommand::ListMembers), "who is there, and who runs it");
    let members = vec![Colleague { user: me.user, name: "olga".into(), role: OrgRole::Admin, pending: false }];
    let effects = session.apply(Action::Server(ServerEvent::Members(members.clone())), 0.0);
    assert!(commands(&effects).contains(&ClientCommand::LoadServerSettings));
    let again = session.apply(Action::Server(ServerEvent::Members(members)), 0.0);
    assert!(!commands(&again).contains(&ClientCommand::LoadServerSettings), "once");
}

#[test]
fn reopening_settings_on_the_server_pane_asks_again() {
    let (mut session, _) = as_owner();
    session.apply(Action::ShowSettingsPane(crate::state::SettingsPane::Server), 0.0);
    session.apply(Action::OpenSettings, 0.0);
    session.apply(Action::CloseSettings, 0.0);
    let effects = session.apply(Action::OpenSettings, 0.0);
    let sent = commands(&effects);
    assert!(sent.contains(&ClientCommand::ListMembers) && sent.contains(&ClientCommand::LoadServerSettings));
}

#[test]
fn the_domain_shown_is_the_servers_and_enter_saves_it() {
    let (mut session, _) = as_owner();
    let shown = ServerSettings { domain: Some("old.acme.io".into()) };
    session.apply(Action::Server(ServerEvent::ServerSettings(shown)), 0.0);
    assert_eq!(session.server_link.domain.text(), "old.acme.io");
    act(&mut session, ServerAction::Focus(ServerField::Domain));
    act(&mut session, ServerAction::Edit(EditKey::Insert("x".into())));
    let sent = commands(&act(&mut session, ServerAction::Submit));
    assert_eq!(sent, [ClientCommand::SaveServerSettings { domain: "old.acme.iox".into() }]);
}

fn urls(base: &str, socket: &str) -> Result<(String, String), AddressProblem> {
    Ok((base.into(), socket.into()))
}

#[test]
fn a_bare_address_is_https_even_with_a_port() {
    assert_eq!(server_urls("superfik.acme.io/", false), urls("https://superfik.acme.io", "wss://superfik.acme.io/ws"));
    assert_eq!(server_urls("192.168.1.5:7420", false), urls("https://192.168.1.5:7420", "wss://192.168.1.5:7420/ws"));
    assert_eq!(server_urls("https://10.0.0.5", false), urls("https://10.0.0.5", "wss://10.0.0.5/ws"));
}

#[test]
fn this_computer_is_plain_http_bare_or_written() {
    assert_eq!(server_urls("localhost:7420", false), urls("http://localhost:7420", "ws://localhost:7420/ws"));
    assert_eq!(server_urls("127.0.0.1:7420", false), urls("http://127.0.0.1:7420", "ws://127.0.0.1:7420/ws"));
    assert_eq!(server_urls("http://[::1]:7420", false), urls("http://[::1]:7420", "ws://[::1]:7420/ws"));
    assert_eq!(
        server_urls("localhost.evil.io", false),
        urls("https://localhost.evil.io", "wss://localhost.evil.io/ws"),
        "only localhost itself"
    );
}

#[test]
fn plain_http_to_another_computer_needs_the_users_say_so() {
    assert_eq!(server_urls("http://10.0.0.5:7420", false), Err(AddressProblem::Insecure));
    assert_eq!(server_urls("http://127.0.0.1.evil.io", false), Err(AddressProblem::Insecure));
    assert_eq!(server_urls("http://10.0.0.5:7420", true), urls("http://10.0.0.5:7420", "ws://10.0.0.5:7420/ws"));
}

#[test]
fn what_is_not_an_address_is_refused() {
    for typed in [" ", "a b", "ftp://acme.io", "ws://acme.io", "https://", "https://ana@acme.io", "acme.io:port", "::1"]
    {
        assert_eq!(server_urls(typed, true), Err(AddressProblem::Invalid), "{typed}");
    }
}

fn filled(session: &mut Session, address: &str) {
    write(session, ServerField::Address, address);
    write(session, ServerField::Name, "ana");
    write(session, ServerField::Password, "correct horse");
}

#[test]
fn plain_http_to_another_computer_asks_first_and_only_for_that_address() {
    let mut session = Session::default();
    filled(&mut session, "http://192.168.1.5:742");
    assert_eq!(joins(&act(&mut session, ServerAction::Join)), None, "the password would travel in clear");
    assert_eq!(session.server_link.problem, Some(AddressProblem::Insecure));
    assert!(!session.server_link.joining);
    let request = joins(&act(&mut session, ServerAction::AllowInsecure)).expect("joins once allowed");
    assert!(request.insecure);
    assert_eq!(session.server_link.problem, None);
    act(&mut session, ServerAction::Joined(Err("refused".into())));
    act(&mut session, ServerAction::Focus(ServerField::Address));
    act(&mut session, ServerAction::Edit(EditKey::Insert("0".into())));
    assert_eq!(joins(&act(&mut session, ServerAction::Join)), None);
    assert_eq!(session.server_link.problem, Some(AddressProblem::Insecure));
}

#[test]
fn https_is_never_marked_insecure_and_a_bad_address_is_said() {
    let mut session = Session::default();
    filled(&mut session, "superfik.acme.io");
    assert!(joins(&act(&mut session, ServerAction::Join)).is_some_and(|request| !request.insecure));
    let mut session = Session::default();
    filled(&mut session, "ftp://acme.io");
    assert_eq!(joins(&act(&mut session, ServerAction::Join)), None);
    assert_eq!(session.server_link.problem, Some(AddressProblem::Invalid));
    assert!(joins(&act(&mut session, ServerAction::AllowInsecure)).is_none(), "nothing to allow");
}

#[test]
fn a_bare_address_that_failed_hints_at_http() {
    let mut session = Session::default();
    filled(&mut session, "192.168.1.5:7420");
    act(&mut session, ServerAction::Join);
    act(&mut session, ServerAction::Joined(Err("could not reach the server".into())));
    assert!(session.server_link.http_hint);
    let mut session = Session::default();
    filled(&mut session, "https://192.168.1.5:7420");
    act(&mut session, ServerAction::Join);
    act(&mut session, ServerAction::Joined(Err("could not reach the server".into())));
    assert!(!session.server_link.http_hint, "the scheme was chosen");
}
