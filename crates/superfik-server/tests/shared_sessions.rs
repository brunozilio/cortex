//! Two members on one project: what one keeps private stays theirs; what
//! one shares reaches the other at once, and on their next load.

use std::sync::Arc;
use std::time::{Duration, Instant};

use superfik_backend::{Backend, InMemoryStore, LocalServices, Memories};
use superfik_core::access::OrgRole;
use superfik_core::project::ProjectRef;
use superfik_core::sharing::Visibility;
use superfik_core::{ConversationId, Effort, Member, Principal, SessionRecord};
use superfik_engine::EngineRegistry;
use superfik_protocol::{ClientCommand, Envelope, ServerEvent};
use superfik_server::testing::{self, admit};
use superfik_server::{Connection, Hosted, Server};

const NOW: u64 = 1_800_000_000;

fn rocket() -> ProjectRef {
    ProjectRef::Remote("github.com/acme/rocket".into())
}

fn record(title: &str, visibility: Visibility) -> SessionRecord {
    SessionRecord {
        id: ConversationId::new(),
        title: Some(title.into()),
        started_unix: 1,
        turns: vec![],
        engine_session: None,
        model: "claude-opus-5-5".into(),
        model_account: None,
        effort: Effort::Medium,
        archived: false,
        repositories: vec![],
        pull_request: None,
        references: vec![],
        agents: vec![],
        research_items: vec![],
        connections: vec![],
        notes: vec![],
        visibility,
    }
}

fn team() -> (Server, Member, Member) {
    let organization = Arc::new(testing::organization(Arc::new(|| NOW)));
    let (olga, _) = admit(&organization, "Olga", OrgRole::Owner);
    let (ana, _) = admit(&organization, "Ana", OrgRole::Member);
    let mut services = LocalServices::with_engines(EngineRegistry::new());
    services.memory = Memories::plain(Arc::new(InMemoryStore::default()));
    services.memory_tool = false;
    let hosted = Hosted::in_memory(organization);
    (Server::start_for(services, Some(hosted)).expect("server"), olga, ana)
}

fn connect(server: &Server, member: Member) -> Connection {
    server.connect(Principal::Member(member), Arc::new(|| {}))
}

fn events(connection: &Connection, count: usize) -> Vec<ServerEvent> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut events = Vec::new();
    while events.len() < count {
        assert!(Instant::now() < deadline, "only {events:?}");
        match connection.try_recv() {
            Some(event) => events.push(event),
            None => std::thread::yield_now(),
        }
    }
    events
}

fn checkout(member: Member) -> std::path::PathBuf {
    superfik_server::checkouts::scratch_checkout(member.user, "rocket")
}

fn save(connection: &Connection, member: Member, session: SessionRecord) {
    let command =
        ClientCommand::SaveSession { path: checkout(member), project: Some(rocket()), session: Box::new(session) };
    connection.send(Envelope::local(command)).expect("send");
}

fn load(connection: &Connection, member: Member) {
    let command = ClientCommand::LoadSessions { path: checkout(member), project: Some(rocket()) };
    connection.send(Envelope::local(command)).expect("send");
}

#[test]
fn a_private_session_stays_with_its_owner() {
    let (server, olga_id, ana_id) = team();
    let (olga, ana) = (connect(&server, olga_id), connect(&server, ana_id));
    save(&olga, olga_id, record("Mine", Visibility::Private));
    let pushed = events(&ana, 1);
    assert!(matches!(&pushed[..], [ServerEvent::SharedSessions { sessions, .. }] if sessions.is_empty()));
    load(&olga, olga_id);
    let loaded = events(&olga, 2);
    assert!(matches!(&loaded[0], ServerEvent::Sessions { sessions, .. } if sessions.len() == 1));
}

#[test]
fn a_shared_session_reaches_the_team_at_once_and_on_load() {
    let (server, olga_id, ana_id) = team();
    let (olga, ana) = (connect(&server, olga_id), connect(&server, ana_id));
    save(&olga, olga_id, record("Launch plan", Visibility::Project));
    let pushed = events(&ana, 1);
    let ServerEvent::SharedSessions { project, sessions } = &pushed[0] else { panic!("not shared") };
    assert_eq!((project, sessions.len()), (&rocket(), 1));
    assert_eq!((sessions[0].owner_name.as_str(), sessions[0].session.title.as_deref()), ("olga", Some("Launch plan")));
    load(&ana, ana_id);
    let loaded = events(&ana, 2);
    assert!(
        matches!(&loaded[0], ServerEvent::Sessions { path, sessions } if sessions.is_empty() && path.ends_with("rocket"))
    );
    assert!(matches!(&loaded[1], ServerEvent::SharedSessions { sessions, .. } if sessions.len() == 1));
}
