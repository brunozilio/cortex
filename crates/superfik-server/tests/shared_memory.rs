//! Ana (member) shares what she learned with the project; Olga (owner)
//! shares a rule with the whole organization. Members cannot write the
//! organization's memory, and only the author or an admin takes one back.

use std::sync::Arc;
use std::time::{Duration, Instant};

use superfik_backend::{Backend, InMemoryStore, LocalServices, Memories};
use superfik_core::access::OrgRole;
use superfik_core::memory::shared::SharedMemory;
use superfik_core::project::ProjectRef;
use superfik_core::sharing::MemoryScope;
use superfik_core::{Author, Member, Memory, MemoryId, MemoryKind, Principal};
use superfik_engine::EngineRegistry;
use superfik_protocol::{ClientCommand, Envelope, ServerEvent};
use superfik_server::testing::{self, admit};
use superfik_server::{Connection, Hosted, Server};

const NOW: u64 = 1_800_000_000;

fn rocket() -> ProjectRef {
    ProjectRef::Remote("github.com/acme/rocket".into())
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

fn memory(text: &str) -> Memory {
    Memory {
        id: MemoryId(40),
        kind: MemoryKind::Procedural,
        text: text.into(),
        tags: vec![],
        created: 1,
        origin: None,
        author: Author::User,
    }
}

fn share(scope: MemoryScope, text: &str) -> ClientCommand {
    ClientCommand::ShareMemory { project: rocket(), scope, memory: memory(text) }
}

fn shared(event: ServerEvent) -> Vec<SharedMemory> {
    let ServerEvent::SharedMemory { project, memories } = event else { panic!("not shared memory: {event:?}") };
    assert_eq!(project, rocket());
    memories
}

#[test]
fn what_one_shares_reaches_everyone_and_the_next_load() {
    let (server, olga, ana) = team();
    let (olga, ana) = (connect(&server, olga), connect(&server, ana));
    send(&ana, share(MemoryScope::Project, "Run cargo xtask lint before pushing."));
    let pushed = shared(next(&olga));
    assert_eq!(shared(next(&ana)), pushed);
    assert_eq!((pushed[0].memory.id, pushed[0].author_name.as_str()), (MemoryId(1), "ana"));
    send(&olga, share(MemoryScope::Organization, "Deploys go through the release train."));
    let _ = next(&ana);
    send(&ana, ClientCommand::LoadSharedMemory { project: rocket() });
    let loaded = shared(next(&ana));
    let texts: Vec<&str> = loaded.iter().map(|m| m.memory.text.as_str()).collect();
    assert_eq!(texts, ["Run cargo xtask lint before pushing.", "Deploys go through the release train."]);
}

#[test]
fn members_cannot_write_the_organizations_memory() {
    let (server, _, ana) = team();
    let ana = connect(&server, ana);
    send(&ana, share(MemoryScope::Organization, "Everyone works on Fridays."));
    assert!(matches!(next(&ana), ServerEvent::Error { .. }));
}

#[test]
fn only_the_author_or_an_admin_takes_one_back() {
    let (server, olga, ana) = team();
    let (olga, ana) = (connect(&server, olga), connect(&server, ana));
    send(&olga, share(MemoryScope::Project, "Use the staging cluster for demos."));
    let _ = (next(&olga), next(&ana));
    let unshare = || ClientCommand::UnshareMemory { project: rocket(), scope: MemoryScope::Project, id: MemoryId(1) };
    send(&ana, unshare());
    assert!(matches!(next(&ana), ServerEvent::Error { .. }));
    send(&olga, unshare());
    assert!(shared(next(&olga)).is_empty());
}
