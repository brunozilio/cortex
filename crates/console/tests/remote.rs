#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, reason = "a failed step fails the test")]

use std::net::SocketAddr;
use std::time::Duration;

use console::remote::{self, Book, Pace, Status};
use console::{Assets, Launch, Live, Settings};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;
use tunnel::{Answer, Ask, Initiator, PairingLink, Payload, StaticKey, Transport};

const WAIT: Duration = Duration::from_secs(10);

type Socket = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

struct World {
    relay: SocketAddr,
    folder: tempfile::TempDir,
    _remote: remote::Remote,
    machine_id: String,
}

fn shell(script: &str) -> Launch {
    Launch { program: "/bin/sh".into(), args: vec!["-c".into(), script.into()], cwd: std::env::temp_dir() }
}

async fn world(terminal: Launch) -> World {
    let relay_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let relay = relay_listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(relay_listener, relay::router(relay::Limits::default(), &[])).await });
    let console_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let console = console_listener.local_addr().unwrap();
    let settings = Settings {
        token: "t".into(),
        port: console.port(),
        assets: Assets::Missing,
        terminal,
        entries: Default::default(),
    };
    let app = console::router(settings, Live::default());
    tokio::spawn(async move { axum::serve(console_listener, app).await });
    let folder = tempfile::tempdir().unwrap();
    let machine_id = Book::in_folder(folder.path()).machine().unwrap().machine_id();
    let pace =
        Pace { idle: Duration::from_millis(50), retry: Duration::from_millis(50), taken: Duration::from_millis(50) };
    let options = remote::Options {
        relay: format!("http://{relay}"),
        folder: folder.path().to_owned(),
        console,
        token: "t".into(),
        pace,
    };
    let remote = remote::start(options).unwrap();
    World { relay, folder, _remote: remote, machine_id }
}

impl World {
    fn book(&self) -> Book {
        Book::in_folder(self.folder.path())
    }

    async fn dial(&self) -> Socket {
        let url = format!("ws://{}/phone?machine={}", self.relay, self.machine_id);
        let deadline = tokio::time::Instant::now() + WAIT;
        while tokio::time::Instant::now() < deadline {
            if let Ok((socket, _)) = tokio_tungstenite::connect_async(&url).await {
                return socket;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        panic!("the machine never reached the relay")
    }

    async fn pair(&self, device: &StaticKey, link: &PairingLink, name: &str) -> Option<Phone> {
        let mut socket = self.dial().await;
        let mut initiator = Initiator::pairing(device, link).unwrap();
        socket.send(Message::Binary(initiator.hello(name.as_bytes()).unwrap().into())).await.unwrap();
        let reply = next_binary(&mut socket).await?;
        let mut phone = Phone { socket, transport: initiator.finish(&reply).ok()? };
        phone.ask(&Ask::Close { id: 0 }).await;
        for _ in 0..200 {
            if self.book().devices().iter().any(|device| device.name == name) {
                return Some(phone);
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        None
    }

    async fn connect(&self, device: &StaticKey) -> Option<Phone> {
        let machine = *self.book().machine().unwrap().public();
        let mut socket = self.dial().await;
        let mut initiator = Initiator::paired(device, &machine).unwrap();
        socket.send(Message::Binary(initiator.hello(&[]).unwrap().into())).await.unwrap();
        let reply = next_binary(&mut socket).await?;
        Some(Phone { socket, transport: initiator.finish(&reply).ok()? })
    }
}

async fn next_binary(socket: &mut Socket) -> Option<Vec<u8>> {
    loop {
        match tokio::time::timeout(WAIT, socket.next()).await.ok()?? {
            Ok(Message::Binary(bytes)) => return Some(bytes.to_vec()),
            Ok(Message::Ping(_) | Message::Pong(_)) => {}
            _ => return None,
        }
    }
}

struct Phone {
    socket: Socket,
    transport: Transport,
}

impl Phone {
    async fn ask(&mut self, ask: &Ask) {
        for piece in self.transport.seal(&serde_json::to_vec(ask).unwrap()).unwrap() {
            self.socket.send(Message::Binary(piece.into())).await.unwrap();
        }
    }

    async fn answer(&mut self) -> Option<Answer> {
        loop {
            let sealed = next_binary(&mut self.socket).await?;
            if let Some(plain) = self.transport.open(&sealed).unwrap() {
                return Some(serde_json::from_slice(&plain).unwrap());
            }
        }
    }

    async fn fetch(&mut self, id: u32, path: &str) -> (u16, String) {
        self.ask(&Ask::Fetch { id, method: "GET".into(), path: path.into(), body: None, content_type: None }).await;
        let mut status = 0;
        let mut body = Vec::new();
        while let Some(answer) = self.answer().await {
            match answer {
                Answer::Head { status: given, .. } => status = given,
                Answer::Data { data, .. } => body.extend(data.decoded().unwrap()),
                Answer::End { .. } => break,
            }
        }
        (status, String::from_utf8_lossy(&body).into_owned())
    }
}

#[tokio::test]
async fn a_phone_pairs_with_a_pending_invitation_and_reaches_the_console_api() {
    let world = world(shell("sleep 5")).await;
    let link = world.book().invite().unwrap();
    let device = StaticKey::generate().unwrap();
    let mut phone = world.pair(&device, &link, "test phone").await.expect("paired");
    let (status, body) = phone.fetch(1, "/api/health").await;
    assert_eq!((status, body.contains("ok")), (200, true), "{body}");
    let names: Vec<String> = world.book().devices().into_iter().map(|device| device.name).collect();
    assert_eq!(names, ["test phone"]);
}

#[tokio::test]
async fn a_paired_phone_comes_back_without_a_new_invitation_and_a_stranger_does_not_get_in() {
    let world = world(shell("sleep 5")).await;
    let device = StaticKey::generate().unwrap();
    let link = world.book().invite().unwrap();
    drop(world.pair(&device, &link, "phone").await.expect("paired"));
    let mut again = world.connect(&device).await.expect("known device");
    assert_eq!(again.fetch(1, "/api/health").await.0, 200);
    assert!(world.connect(&StaticKey::generate().unwrap()).await.is_none());
}

#[tokio::test]
async fn a_device_that_saw_the_pairing_id_but_not_the_code_is_never_paired() {
    use sha2::Digest;
    let world = world(shell("sleep 5")).await;
    let link = world.book().invite().unwrap();
    let mut header = vec![2u8];
    header.extend_from_slice(&link.pairing_id());
    let guessed: [u8; 32] = sha2::Sha256::digest(b"not the code").into();
    let intruder = StaticKey::generate().unwrap();
    let mut state = snow::Builder::new("Noise_IKpsk2_25519_ChaChaPoly_SHA256".parse().unwrap())
        .local_private_key(intruder.private())
        .unwrap()
        .remote_public_key(&link.machine)
        .unwrap()
        .prologue(&header)
        .unwrap()
        .psk(2, &guessed)
        .unwrap()
        .build_initiator()
        .unwrap();
    let mut message = vec![0; 1024];
    let written = state.write_message(b"intruder", &mut message).unwrap();
    let mut hello = header.clone();
    hello.extend_from_slice(&message[..written]);
    let mut socket = world.dial().await;
    socket.send(Message::Binary(hello.into())).await.unwrap();
    if let Some(reply) = next_binary(&mut socket).await {
        assert!(state.read_message(&reply, &mut vec![0; 1024]).is_err(), "the reply needs the code");
    }
    let forged = [0u8; 64];
    let _ = socket.send(Message::Binary(forged.to_vec().into())).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(world.book().devices().is_empty(), "the intruder was recorded as a device");
    assert!(world.connect(&intruder).await.is_none(), "the intruder got in as a paired device");
    assert!(
        world.pair(&StaticKey::generate().unwrap(), &link, "owner").await.is_some(),
        "the invitation was not spent"
    );
}

#[tokio::test]
async fn a_relay_cannot_open_more_channels_than_the_machine_serves_at_once() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let folder = tempfile::tempdir().unwrap();
    Book::in_folder(folder.path()).invite().unwrap();
    let pace = Pace { idle: Duration::from_millis(50), retry: Duration::from_secs(60), taken: Duration::from_secs(60) };
    let options = remote::Options {
        relay: format!("http://{address}"),
        folder: folder.path().to_owned(),
        console: "127.0.0.1:9".parse().unwrap(),
        token: "t".into(),
        pace,
    };
    let _remote = remote::start(options).unwrap();
    let (tcp, _) = tokio::time::timeout(WAIT, listener.accept()).await.unwrap().unwrap();
    let mut machine = tokio_tungstenite::accept_async(tcp).await.unwrap();
    let challenge = tunnel::proof::Challenge::new().unwrap();
    machine.send(Message::Text(challenge.text().into())).await.unwrap();
    let Some(Ok(Message::Text(proof))) = machine.next().await else { panic!("no proof") };
    let id = Book::in_folder(folder.path()).machine().unwrap().machine_id();
    assert_eq!(challenge.check(&id, proof.as_str()), Ok(()), "the machine proves the key behind its id");
    machine.send(Message::Text(tunnel::proof::PROVEN.into())).await.unwrap();
    let extra = 5;
    let opened = u32::try_from(remote::MOST_CHANNELS + extra).unwrap();
    for channel in 1..=opened {
        machine.send(Message::Binary(tunnel::Frame::Open(channel).encode().into())).await.unwrap();
    }
    let mut closed = Vec::new();
    while closed.len() < extra {
        let bytes = match tokio::time::timeout(WAIT, machine.next()).await {
            Ok(Some(Ok(Message::Binary(bytes)))) => bytes,
            Ok(Some(Ok(_))) => continue,
            _ => break,
        };
        if let Ok(tunnel::Frame::Close(channel)) = tunnel::Frame::decode(&bytes) {
            closed.push(channel);
        }
    }
    let expected: Vec<u32> = (opened - u32::try_from(extra).unwrap() + 1..=opened).collect();
    assert_eq!(closed, expected, "the channels past the cap are closed at once, the others wait for a hello");
}

#[tokio::test]
async fn a_relay_older_than_the_machine_is_reported_with_how_to_update_it() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let folder = tempfile::tempdir().unwrap();
    Book::in_folder(folder.path()).invite().unwrap();
    let pace = Pace { idle: Duration::from_millis(50), retry: Duration::from_secs(60), taken: Duration::from_secs(60) };
    let options = remote::Options {
        relay: format!("http://{address}"),
        folder: folder.path().to_owned(),
        console: "127.0.0.1:9".parse().unwrap(),
        token: "t".into(),
        pace,
    };
    let remote = remote::start(options).unwrap();
    let (tcp, _) = tokio::time::timeout(WAIT, listener.accept()).await.unwrap().unwrap();
    let mut old = tokio_tungstenite::accept_async(tcp).await.unwrap();
    old.send(Message::Binary(tunnel::Frame::Open(1).encode().into())).await.unwrap();
    let mut watch = remote.watch();
    let failed = tokio::time::timeout(WAIT, watch.wait_for(|status| matches!(status, Status::Failed(_)))).await;
    let Ok(Ok(status)) = failed else { panic!("no failure reported") };
    assert_eq!(*status, Status::Failed(remote::OUTDATED_RELAY.to_owned()));
    assert!(remote::OUTDATED_RELAY.contains("remote update"));
}

#[tokio::test]
async fn an_invitation_is_used_once() {
    let world = world(shell("sleep 5")).await;
    let link = world.book().invite().unwrap();
    assert!(world.pair(&StaticKey::generate().unwrap(), &link, "first").await.is_some());
    assert!(world.pair(&StaticKey::generate().unwrap(), &link, "second").await.is_none());
}

#[tokio::test]
async fn only_the_console_api_is_forwarded() {
    let world = world(shell("sleep 5")).await;
    let link = world.book().invite().unwrap();
    let mut phone = world.pair(&StaticKey::generate().unwrap(), &link, "phone").await.unwrap();
    assert_eq!(phone.fetch(1, "/index.html").await.0, 403);
    assert_eq!(phone.fetch(2, "/api/../index.html").await.0, 403);
}

#[tokio::test]
async fn the_phone_types_into_the_terminal_and_sees_what_it_prints() {
    let world = world(shell("printf ready; read line; printf \"got:%s\" \"$line\"; sleep 1")).await;
    let link = world.book().invite().unwrap();
    let mut phone = world.pair(&StaticKey::generate().unwrap(), &link, "phone").await.unwrap();
    phone.ask(&Ask::Socket { id: 7, path: "/api/terminal?cols=80&rows=24".into() }).await;
    assert!(matches!(phone.answer().await, Some(Answer::Head { id: 7, status: 101, .. })));
    let mut seen = String::new();
    let mut typed = false;
    while let Some(Answer::Data { data, .. }) = phone.answer().await {
        seen.push_str(&String::from_utf8_lossy(&data.decoded().unwrap()));
        if seen.contains("ready") && !typed {
            phone.ask(&Ask::Send { id: 7, data: Payload::bytes(b"hello\r") }).await;
            typed = true;
        }
        if seen.contains("got:hello") {
            return;
        }
    }
    panic!("never saw the answer: {seen:?}");
}

#[tokio::test]
async fn a_revoked_phone_is_cut_off() {
    let world = world(shell("sleep 5")).await;
    let link = world.book().invite().unwrap();
    let mut phone = world.pair(&StaticKey::generate().unwrap(), &link, "phone").await.unwrap();
    world.book().revoke("phone").unwrap();
    assert_eq!(next_binary(&mut phone.socket).await, None);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_paste_into_a_busy_terminal_never_freezes_the_link_and_a_revoked_phone_is_still_cut_off() {
    let world = world(shell("yes")).await;
    let link = world.book().invite().unwrap();
    let mut phone = world.pair(&StaticKey::generate().unwrap(), &link, "phone").await.unwrap();
    phone.ask(&Ask::Socket { id: 7, path: "/api/terminal?cols=80&rows=24".into() }).await;
    assert!(matches!(phone.answer().await, Some(Answer::Head { id: 7, status: 101, .. })));
    let Phone { socket, mut transport } = phone;
    let (mut sink, mut stream) = socket.split();
    let (closed, seen_closed) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
        let ended = loop {
            match tokio::time::timeout_at(deadline, stream.next()).await {
                Err(_) => break false,
                Ok(None | Some(Err(_) | Ok(Message::Close(_)))) => break true,
                Ok(Some(Ok(_))) => {}
            }
        };
        let _ = closed.send(ended);
    });
    let paste = serde_json::to_vec(&Ask::Send { id: 7, data: Payload::bytes(&[b'x'; 8 * 1024]) }).unwrap();
    let flood = async {
        for _ in 0..2_000 {
            for piece in transport.seal(&paste).unwrap() {
                if sink.send(Message::Binary(piece.into())).await.is_err() {
                    return;
                }
            }
        }
    };
    let _ = tokio::time::timeout(Duration::from_secs(5), flood).await;
    world.book().revoke("phone").unwrap();
    let cut = tokio::time::timeout(WAIT + Duration::from_secs(5), seen_closed).await;
    assert!(matches!(cut, Ok(Ok(true))), "the link froze behind the paste and the revocation never landed");
}

#[tokio::test]
async fn the_machine_waits_off_the_relay_until_someone_can_be_served() {
    let world = world(shell("sleep 5")).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(world._remote.status(), Status::Waiting);
    let url = format!("ws://{}/phone?machine={}", world.relay, world.machine_id);
    assert!(tokio_tungstenite::connect_async(&url).await.is_err());
    world.book().invite().unwrap();
    drop(world.dial().await);
    let mut watch = world._remote.watch();
    assert!(tokio::time::timeout(WAIT, watch.wait_for(|status| *status == Status::Connected)).await.is_ok());
}

#[test]
fn a_second_machine_with_the_same_key_is_told_the_place_is_taken() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let world = world(shell("sleep 5")).await;
        world.book().invite().unwrap();
        drop(world.dial().await);
        let options = remote::Options {
            relay: format!("http://{}", world.relay),
            folder: world.folder.path().to_owned(),
            console: "127.0.0.1:9".parse().unwrap(),
            token: "t".into(),
            pace: Pace {
                idle: Duration::from_millis(50),
                retry: Duration::from_millis(50),
                taken: Duration::from_secs(60),
            },
        };
        let second = remote::start(options).unwrap();
        let mut watch = second.watch();
        let taken = tokio::time::timeout(WAIT, watch.wait_for(|status| *status == Status::Taken)).await;
        assert!(taken.is_ok());
    });
}
