//! The app's backend against a real server on a loopback port.

use std::sync::Arc;
use std::time::{Duration, Instant};

use superfik_backend::{Backend, InMemoryStore, LocalServices, Memories, Wake};
use superfik_core::{EngineEvent, EngineKind, ModelOption, OrgId, Principal, UserId};
use superfik_engine::EngineRegistry;
use superfik_engine::fake::FakeEngine;
use superfik_protocol::{ClientCommand, Envelope, PROTOCOL_VERSION, ServerEvent};
use superfik_remote::{RemoteBackend, RemoteError};
use superfik_server::{Authenticator, Server};

struct OneToken(Principal);

impl Authenticator for OneToken {
    fn authenticate(&self, token: &str) -> Option<Principal> {
        (token == "secret").then(|| self.0.clone())
    }
}

fn serve(member: Principal) -> String {
    let model = ModelOption::new(EngineKind::Native, "claude-opus-5-5", "Opus 5.5");
    let engine = FakeEngine::new(EngineKind::Native, vec![model], vec![EngineEvent::Cancelled]);
    let mut services = LocalServices::with_engines(EngineRegistry::new().with(Arc::new(engine)));
    services.memory = Memories::plain(Arc::new(InMemoryStore::default()));
    services.memory_tool = false;
    services.authorizer = Arc::new(superfik_backend::RoleAuthorizer::new(OrgId::LOCAL, Everyone));
    let server = Server::start(services).expect("server");
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let listener = runtime.block_on(tokio::net::TcpListener::bind("127.0.0.1:0")).expect("bind");
    let address = listener.local_addr().expect("address");
    let routes = superfik_server::socket(server, Arc::new(OneToken(member)));
    std::thread::spawn(move || runtime.block_on(superfik_server::serve(listener, routes)));
    format!("ws://{address}/ws")
}

struct Everyone;

impl superfik_backend::Roles for Everyone {
    fn org_role(&self, _: &superfik_core::Member) -> Option<superfik_core::access::OrgRole> {
        Some(superfik_core::access::OrgRole::Owner)
    }
    fn project_role(&self, _: &superfik_core::Member, _: &ClientCommand) -> Option<superfik_core::access::ProjectRole> {
        None
    }
}

fn wake() -> (Wake, crossbeam_channel::Receiver<()>) {
    let (tx, rx) = crossbeam_channel::unbounded();
    (
        Arc::new(move || {
            let _ = tx.send(());
        }),
        rx,
    )
}

fn next(backend: &RemoteBackend, woke: &crossbeam_channel::Receiver<()>) -> ServerEvent {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if let Some(event) = backend.try_recv() {
            return event;
        }
        let _ = woke.recv_timeout(Duration::from_millis(50));
    }
    panic!("no event");
}

#[test]
fn a_member_is_welcomed_as_their_token_says() {
    let member = Principal::member(UserId::new(), OrgId::LOCAL);
    let url = serve(member.clone());
    let (wake, woke) = wake();
    let backend = RemoteBackend::connect(&url, "secret", wake).expect("connect");
    backend.send(Envelope::local(ClientCommand::Hello { version: PROTOCOL_VERSION })).expect("send");
    assert_eq!(next(&backend, &woke), ServerEvent::Welcome { version: PROTOCOL_VERSION, principal: member });
}

#[test]
fn a_wrong_token_is_refused() {
    let url = serve(Principal::member(UserId::new(), OrgId::LOCAL));
    let (wake, _woke) = wake();
    assert!(matches!(RemoteBackend::connect(&url, "guess", wake), Err(RemoteError::Refused)));
}
