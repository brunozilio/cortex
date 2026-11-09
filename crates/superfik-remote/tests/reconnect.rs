//! The app's backend reconnects on its own when the connection drops
//! (ADR 0010), greets the server again, and stops when the server no
//! longer takes its sign-in.

use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use superfik_backend::{Backend, InMemoryStore, LocalServices, Memories, Wake};
use superfik_core::{OrgId, Principal, UserId};
use superfik_engine::EngineRegistry;
use superfik_protocol::{ClientCommand, Envelope, LinkState, PROTOCOL_VERSION, ServerEvent};
use superfik_remote::{Credentials, RemoteBackend};
use superfik_server::{Authenticator, Server};

struct OneToken(Principal);

impl Authenticator for OneToken {
    fn authenticate(&self, token: &str) -> Option<Principal> {
        (token == "secret").then(|| self.0.clone())
    }
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

fn serve(member: Principal) -> std::net::SocketAddr {
    let mut services = LocalServices::with_engines(EngineRegistry::new());
    services.memory = Memories::plain(Arc::new(InMemoryStore::default()));
    services.memory_tool = false;
    services.authorizer = Arc::new(superfik_backend::RoleAuthorizer::new(OrgId::LOCAL, Everyone));
    let server = Server::start(services).expect("server");
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let listener = runtime.block_on(tokio::net::TcpListener::bind("127.0.0.1:0")).expect("bind");
    let address = listener.local_addr().expect("address");
    let routes = superfik_server::socket(server, Arc::new(OneToken(member)));
    std::thread::spawn(move || runtime.block_on(superfik_server::serve(listener, routes)));
    address
}

#[derive(Clone, Default)]
struct Relay(Arc<Mutex<Vec<TcpStream>>>);

impl Relay {
    fn start(upstream: std::net::SocketAddr) -> (Self, String) {
        let relay = Self::default();
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let url = format!("ws://{}/ws", listener.local_addr().expect("address"));
        let open = relay.clone();
        std::thread::spawn(move || {
            for client in listener.incoming().flatten() {
                let Ok(server) = TcpStream::connect(upstream) else { continue };
                open.0
                    .lock()
                    .expect("lock")
                    .extend([client.try_clone().expect("clone"), server.try_clone().expect("clone")]);
                pipe(client.try_clone().expect("clone"), server.try_clone().expect("clone"));
                pipe(server, client);
            }
        });
        (relay, url)
    }

    fn cut(&self) {
        for stream in self.0.lock().expect("lock").drain(..) {
            let _ = stream.shutdown(Shutdown::Both);
        }
    }
}

fn pipe(mut from: TcpStream, mut to: TcpStream) {
    std::thread::spawn(move || {
        let mut buffer = [0; 4096];
        while let Ok(read) = from.read(&mut buffer) {
            if read == 0 || to.write_all(&buffer[..read]).is_err() {
                break;
            }
        }
        let _ = to.shutdown(Shutdown::Both);
    });
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
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if let Some(event) = backend.try_recv() {
            return event;
        }
        let _ = woke.recv_timeout(Duration::from_millis(50));
    }
    panic!("no event");
}

fn hello() -> Envelope<ClientCommand> {
    Envelope::local(ClientCommand::Hello { version: PROTOCOL_VERSION })
}

#[test]
fn a_dropped_connection_comes_back_greeted_again() {
    let member = Principal::member(UserId::new(), OrgId::LOCAL);
    let (relay, url) = Relay::start(serve(member.clone()));
    let (wake, woke) = wake();
    let backend = RemoteBackend::connect(&url, "secret", wake).expect("connect");
    backend.send(hello()).expect("send");
    let welcome = ServerEvent::Welcome { version: PROTOCOL_VERSION, principal: member };
    assert_eq!(next(&backend, &woke), welcome);
    relay.cut();
    assert_eq!(next(&backend, &woke), ServerEvent::Link(LinkState::Lost));
    assert_eq!(next(&backend, &woke), ServerEvent::Link(LinkState::Restored));
    assert_eq!(next(&backend, &woke), welcome, "greeted anew on its own");
}

#[test]
fn a_sign_in_the_server_no_longer_takes_ends_the_link() {
    let member = Principal::member(UserId::new(), OrgId::LOCAL);
    let (relay, url) = Relay::start(serve(member.clone()));
    let (wake, woke) = wake();
    let asked = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&asked);
    let credentials: Credentials = Arc::new(move || {
        Some(if counter.fetch_add(1, Ordering::SeqCst) == 0 { "secret" } else { "expired" }.to_owned())
    });
    let backend = RemoteBackend::connect_with(&url, credentials, wake).expect("connect");
    backend.send(hello()).expect("send");
    next(&backend, &woke);
    relay.cut();
    assert_eq!(next(&backend, &woke), ServerEvent::Link(LinkState::Lost));
    assert_eq!(next(&backend, &woke), ServerEvent::Link(LinkState::SignedOut));
    assert!(asked.load(Ordering::SeqCst) >= 2, "asked for a fresh token");
}
