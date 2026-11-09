//! Olga (owner) makes a Design team with Ana in it and shares a session
//! with it: Ana sees it, Rui (not in the team) does not. Members cannot
//! change teams.

use std::sync::Arc;
use std::time::{Duration, Instant};

use superfik_backend::{Backend, InMemoryStore, LocalServices, Memories};
use superfik_core::access::OrgRole;
use superfik_core::project::ProjectRef;
use superfik_core::sharing::{Audience, Visibility};
use superfik_core::{ConversationId, Effort, Member, Principal, SessionRecord};
use superfik_engine::EngineRegistry;
use superfik_protocol::{ClientCommand, Envelope, ServerEvent};
use superfik_server::testing::{self, admit};
use superfik_server::{Connection, Hosted, Server};

const NOW: u64 = 1_800_000_000;

fn team() -> (Server, Member, Member, Member) {
    let organization = Arc::new(testing::organization(Arc::new(|| NOW)));
    let olga = admit(&organization, "Olga", OrgRole::Owner).0;
    let (ana, rui) = (admit(&organization, "Ana", OrgRole::Member).0, admit(&organization, "Rui", OrgRole::Member).0);
    let mut services = LocalServices::with_engines(EngineRegistry::new());
    services.memory = Memories::plain(Arc::new(InMemoryStore::default()));
    services.memory_tool = false;
    (Server::start_for(services, Some(Hosted::in_memory(organization))).expect("server"), olga, ana, rui)
}

fn connect(server: &Server, member: Member) -> Connection {
    server.connect(Principal::Member(member), Arc::new(|| {}))
}

fn send(connection: &Connection, command: ClientCommand) {
    connection.send(Envelope::local(command)).expect("send");
}

fn next(connection: &Connection) -> ServerEvent {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(event) = connection.try_recv() {
            return event;
        }
        assert!(Instant::now() < deadline, "nothing arrived");
        std::thread::yield_now();
    }
}

fn rocket() -> ProjectRef {
    ProjectRef::Remote("github.com/acme/rocket".into())
}

fn record(visibility: Visibility) -> SessionRecord {
    SessionRecord {
        id: ConversationId::new(),
        title: Some("Redesign".into()),
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

fn shared_count(event: ServerEvent) -> usize {
    let ServerEvent::SharedSessions { sessions, .. } = event else { panic!("not shared sessions: {event:?}") };
    sessions.len()
}

#[test]
fn a_session_shared_with_a_team_reaches_its_members_only() {
    let (server, olga_id, ana_id, rui_id) = team();
    let (olga, ana, rui) = (connect(&server, olga_id), connect(&server, ana_id), connect(&server, rui_id));
    send(&ana, ClientCommand::CreateTeam { name: "Design".into() });
    assert!(matches!(next(&ana), ServerEvent::Error { .. }), "members do not make teams");
    send(&olga, ClientCommand::CreateTeam { name: "Design".into() });
    let ServerEvent::Teams(teams) = next(&olga) else { panic!("no teams") };
    let (_, _) = (next(&ana), next(&rui));
    let design = teams[0].id;
    send(&olga, ClientCommand::SetTeamMember { team: design, user: ana_id.user, member: true });
    for connection in [&olga, &ana, &rui] {
        let ServerEvent::Teams(teams) = next(connection) else { panic!("no teams") };
        assert_eq!(teams[0].members, [ana_id.user]);
    }
    let session = record(Visibility::Shared(vec![Audience::Team(design)]));
    let save = ClientCommand::SaveSession {
        path: superfik_server::checkouts::scratch_checkout(olga_id.user, "rocket"),
        project: Some(rocket()),
        session: Box::new(session),
    };
    send(&olga, save);
    assert_eq!(shared_count(next(&ana)), 1, "in the team");
    assert_eq!(shared_count(next(&rui)), 0, "not in it");
}
