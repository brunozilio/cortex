//! Joins a running `superfik-server` the way the app does once signed in:
//! open the socket with the sign-in token the server issued, say hello.
//! Get a token with `curl -X POST <server>/auth/sign-in -d
//! '{"name":"…","password":"…"}' -H 'content-type: application/json'`:
//!
//! ```sh
//! SUPERFIK_LIVE_SOCKET=ws://localhost:7420/ws SUPERFIK_LIVE_TOKEN=… \
//!     cargo test -p superfik-remote --test live_server -- --ignored
//! ```

use std::sync::Arc;
use std::time::{Duration, Instant};

use superfik_backend::{Backend, Wake};
use superfik_protocol::{ClientCommand, Envelope, PROTOCOL_VERSION, ServerEvent};
use superfik_remote::RemoteBackend;

fn var(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} is not set"))
}

#[test]
#[ignore = "needs a running server and a sign-in token"]
fn joins_a_running_server_with_its_sign_in_token() {
    let (tx, woke) = crossbeam_channel::unbounded();
    let wake: Wake = Arc::new(move || {
        let _ = tx.send(());
    });
    let token = var("SUPERFIK_LIVE_TOKEN");
    let backend = RemoteBackend::connect(&var("SUPERFIK_LIVE_SOCKET"), &token, wake).expect("connect");
    backend.send(Envelope::local(ClientCommand::Hello { version: PROTOCOL_VERSION })).expect("send");
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        match backend.try_recv() {
            Some(ServerEvent::Welcome { version, principal }) => {
                assert_eq!(version, PROTOCOL_VERSION);
                assert!(matches!(principal, superfik_core::Principal::Member(_)), "welcomed as {principal:?}");
                return;
            }
            Some(_) => {}
            None => {
                let _ = woke.recv_timeout(Duration::from_millis(50));
            }
        }
    }
    panic!("no welcome");
}
