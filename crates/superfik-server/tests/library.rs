//! Olga (owner) publishes a skill; Ana (member) receives the library and
//! cannot publish or remove.

use std::sync::Arc;
use std::time::{Duration, Instant};

use superfik_backend::{Backend, InMemoryStore, LocalServices, Memories};
use superfik_core::access::OrgRole;
use superfik_core::library::{LibraryItem, LibraryKind};
use superfik_core::{Member, Principal};
use superfik_engine::EngineRegistry;
use superfik_protocol::{ClientCommand, Envelope, ServerEvent};
use superfik_server::testing::{self, admit};
use superfik_server::{Connection, Hosted, Server};

const NOW: u64 = 1_800_000_000;

fn team() -> (Server, Member, Member) {
    let (server, olga, ana, _) = with_guest();
    (server, olga, ana)
}

fn with_guest() -> (Server, Member, Member, Member) {
    let organization = Arc::new(testing::organization(Arc::new(|| NOW)));
    let olga = admit(&organization, "Olga", OrgRole::Owner).0;
    let ana = admit(&organization, "Ana", OrgRole::Member).0;
    let gil = admit(&organization, "Gil", OrgRole::Guest).0;
    let mut services = LocalServices::with_engines(EngineRegistry::new());
    services.memory = Memories::plain(Arc::new(InMemoryStore::default()));
    services.memory_tool = false;
    let hosted = Hosted::in_memory(organization);
    (Server::start_for(services, Some(hosted)).expect("server"), olga, ana, gil)
}

#[test]
fn guests_neither_hear_nor_list_the_library() {
    let (server, olga, ana, gil) = with_guest();
    let (olga, ana, gil) = (connect(&server, olga), connect(&server, ana), connect(&server, gil));
    send(&olga, ClientCommand::PublishToLibrary { item: release() });
    let _ = (next(&olga), next(&ana));
    send(&gil, ClientCommand::ListLibrary);
    assert!(matches!(next(&gil), ServerEvent::Error { .. }), "the error, not the publish before it");
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

fn release() -> LibraryItem {
    LibraryItem {
        kind: LibraryKind::Skill,
        name: "release".into(),
        body: "---\nname: release\n---".into(),
        author_name: "anyone".into(),
    }
}

#[test]
fn an_owner_publishes_and_everyone_gets_the_library() {
    let (server, olga, ana) = team();
    let (olga, ana) = (connect(&server, olga), connect(&server, ana));
    send(&olga, ClientCommand::PublishToLibrary { item: release() });
    let ServerEvent::Library(items) = next(&ana) else { panic!("no library") };
    assert_eq!((items.len(), items[0].author_name.as_str()), (1, "olga"), "the server names the author");
    assert!(matches!(next(&olga), ServerEvent::Library(_)));
    send(&ana, ClientCommand::ListLibrary);
    assert!(matches!(next(&ana), ServerEvent::Library(items) if items.len() == 1));
}

#[test]
fn a_member_neither_publishes_nor_removes() {
    let (server, olga, ana) = team();
    let (olga, ana) = (connect(&server, olga), connect(&server, ana));
    send(&ana, ClientCommand::PublishToLibrary { item: release() });
    assert!(matches!(next(&ana), ServerEvent::Error { .. }));
    send(&olga, ClientCommand::PublishToLibrary { item: release() });
    let _ = (next(&olga), next(&ana));
    send(&ana, ClientCommand::RemoveFromLibrary { kind: LibraryKind::Skill, name: "release".into() });
    assert!(matches!(next(&ana), ServerEvent::Error { .. }));
}
