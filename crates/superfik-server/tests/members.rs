//! Users through a hosted server's connection: owners and admins add them
//! with a first-access code, change roles, remove them and set how the
//! server is reached; everyone lists them.

use std::sync::Arc;
use std::time::{Duration, Instant};

use superfik_backend::{Backend, InMemoryStore, LocalServices, Memories};
use superfik_core::access::OrgRole;
use superfik_core::{Member, OrgId, Principal};
use superfik_engine::EngineRegistry;
use superfik_protocol::{ClientCommand, Envelope, ServerEvent, ServerSettings};
use superfik_server::testing::{self, admit};
use superfik_server::{Authenticator, Connection, Hosted, Organization, Server};

const NOW: u64 = 1_800_000_000;

fn organization() -> (Arc<Organization>, Server, Member) {
    let organization = Arc::new(testing::organization(Arc::new(|| NOW)));
    let (owner, _) = admit(&organization, "Olga", OrgRole::Owner);
    let mut services = LocalServices::with_engines(EngineRegistry::new());
    services.memory = Memories::plain(Arc::new(InMemoryStore::default()));
    services.memory_tool = false;
    let hosted = Hosted::in_memory(Arc::clone(&organization));
    let server = Server::start_for(services, Some(hosted)).expect("server");
    (organization, server, owner)
}

fn connect(server: &Server, member: Member) -> Connection {
    server.connect(Principal::Member(member), Arc::new(|| {}))
}

fn next(connection: &Connection) -> ServerEvent {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(event) = connection.try_recv() {
            return event;
        }
        assert!(Instant::now() < deadline, "no event");
        std::thread::yield_now();
    }
}

fn send(connection: &Connection, command: ClientCommand) {
    connection.send(Envelope::local(command)).expect("send");
}

#[test]
fn users_are_listed_with_their_role_and_whether_they_made_their_first_access() {
    let (organization, server, owner) = organization();
    let (ana, _) = admit(&organization, "Ana", OrgRole::Admin);
    organization.directory.add(superfik_identity::Actor::Console, "bia", OrgRole::Member, NOW).expect("bia");
    let olga = connect(&server, owner);
    send(&olga, ClientCommand::ListMembers);
    let ServerEvent::Members(members) = next(&olga) else { panic!("no members") };
    let names: Vec<(&str, bool)> = members.iter().map(|m| (m.name.as_str(), m.pending)).collect();
    assert_eq!(names, [("ana", false), ("bia", true), ("olga", false)]);
    assert!(members.iter().any(|m| m.user == ana.user && m.role == OrgRole::Admin));
}

#[test]
fn an_admin_adds_a_user_who_signs_in_with_the_code_once() {
    let (organization, server, owner) = organization();
    let olga = connect(&server, owner);
    send(&olga, ClientCommand::AddUser { name: "Bia".into(), role: OrgRole::Member });
    let ServerEvent::FirstAccessCode { name, code, user } = next(&olga) else { panic!("no code") };
    assert_eq!(name, "bia");
    assert!(matches!(next(&olga), ServerEvent::Members(members) if members.len() == 2), "everyone's list follows");
    let token = organization.directory.first_access("bia", &code, testing::PASSWORD, NOW).expect("first access");
    assert_eq!(organization.authenticate(&token), Some(Principal::member(user, organization.directory.org())));
}

#[test]
fn a_new_code_ends_a_users_sign_ins_and_removal_ends_them_for_good() {
    let (organization, server, owner) = organization();
    let (ana, token) = admit(&organization, "Ana", OrgRole::Member);
    let olga = connect(&server, owner);
    send(&olga, ClientCommand::NewFirstAccessCode { user: ana.user });
    assert!(matches!(next(&olga), ServerEvent::FirstAccessCode { .. }));
    assert!(matches!(next(&olga), ServerEvent::Members(members) if members.iter().any(|m| m.pending)));
    assert_eq!(organization.authenticate(&token), None);
    send(&olga, ClientCommand::RemoveUser { user: ana.user });
    assert!(matches!(next(&olga), ServerEvent::Members(members) if members.len() == 1));
    assert_eq!(organization.directory.role(&ana), None);
}

#[test]
fn members_list_but_cannot_change_users_or_settings() {
    let (organization, server, _) = organization();
    let (ana, _) = admit(&organization, "Ana", OrgRole::Member);
    let ana = connect(&server, ana);
    for command in [
        ClientCommand::AddUser { name: "eve".into(), role: OrgRole::Owner },
        ClientCommand::LoadServerSettings,
        ClientCommand::SaveServerSettings { domain: "evil.io".into() },
    ] {
        send(&ana, command);
        assert!(matches!(next(&ana), ServerEvent::Error { .. }));
    }
    assert_eq!(organization.settings().domain, None);
}

#[test]
fn owners_set_the_domain_and_it_holds_at_once() {
    let (organization, server, owner) = organization();
    let olga = connect(&server, owner);
    send(&olga, ClientCommand::SaveServerSettings { domain: " superfik.acme.io ".into() });
    let shown = ServerSettings { domain: Some("superfik.acme.io".into()) };
    assert_eq!(next(&olga), ServerEvent::ServerSettings(shown));
    assert_eq!(organization.origin().as_deref(), Some("https://superfik.acme.io"));
    send(&olga, ClientCommand::SaveServerSettings { domain: String::new() });
    assert_eq!(next(&olga), ServerEvent::ServerSettings(ServerSettings { domain: None }));
    assert_eq!(organization.origin(), None);
}

#[test]
fn strangers_list_nothing() {
    let (_organization, server, _) = organization();
    let stranger = Member { user: superfik_core::UserId::new(), org: OrgId::new() };
    let stranger = connect(&server, stranger);
    send(&stranger, ClientCommand::ListMembers);
    assert!(matches!(next(&stranger), ServerEvent::Error { .. }));
}
