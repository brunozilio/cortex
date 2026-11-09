//! Olga shares a session with Ana alone; Ana comments on it. Olga and Ana
//! hear the comment, Rui neither hears it nor can read or add any.

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

fn checkout(member: Member) -> std::path::PathBuf {
    superfik_server::checkouts::scratch_checkout(member.user, "rocket")
}

fn rocket() -> ProjectRef {
    ProjectRef::Remote("github.com/acme/rocket".into())
}

#[test]
fn only_who_sees_a_session_reads_and_hears_its_comments() {
    let (server, olga_id, ana_id, rui_id) = team();
    let (olga, ana, rui) = (connect(&server, olga_id), connect(&server, ana_id), connect(&server, rui_id));
    let session = SessionRecord {
        id: ConversationId::new(),
        title: Some("Launch".into()),
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
        visibility: Visibility::Shared(vec![Audience::Member(ana_id.user)]),
    };
    let id = session.id;
    let save =
        ClientCommand::SaveSession { path: checkout(olga_id), project: Some(rocket()), session: Box::new(session) };
    send(&olga, save);
    let (_, _) = (next(&ana), next(&rui));
    let comment =
        |text: &str| ClientCommand::AddComment { project: rocket(), session: id, after_turn: 0, text: text.into() };
    send(&rui, comment("Me too"));
    assert!(matches!(next(&rui), ServerEvent::Error { .. }), "Rui cannot see it");
    send(&ana, comment("  Ship it  "));
    for connection in [&olga, &ana] {
        let ServerEvent::Comments { session, comments } = next(connection) else { panic!("no comments") };
        assert_eq!(session, id);
        assert_eq!((comments[0].author_name.as_str(), comments[0].text.as_str()), ("ana", "Ship it"));
    }
    send(&rui, ClientCommand::LoadComments { project: rocket(), session: id });
    assert!(matches!(next(&rui), ServerEvent::Error { .. }));
    send(&olga, ClientCommand::LoadComments { project: rocket(), session: id });
    assert!(matches!(next(&olga), ServerEvent::Comments { comments, .. } if comments.len() == 1));
}
