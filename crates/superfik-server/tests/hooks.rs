//! `POST /hooks/<id>` on the server: the routine's token as a bearer fires
//! it; anything else learns only a status.

use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use superfik_backend::{Backend, InMemorySecretStore, InMemoryStore, LocalServices, Memories, Routines};
use superfik_core::routine::{
    Autonomy, MAX_WEBHOOK_BYTES, RoutineDraft, RoutineEdit, RoutineId, Schedule, WebhookId, WebhookToken,
};
use superfik_core::{EngineEvent, EngineKind, ModelOption, Principal};
use superfik_engine::EngineRegistry;
use superfik_engine::fake::FakeEngine;
use superfik_protocol::{ClientCommand, Envelope, ServerEvent};
use superfik_server::{Connection, Server};
use tower::ServiceExt;

fn server() -> Server {
    let model = ModelOption::new(EngineKind::Native, "claude-opus-5-5", "Opus 5.5");
    let script = vec![EngineEvent::Completed { duration_ms: 1, cost_usd: None }];
    let engine = FakeEngine::new(EngineKind::Native, vec![model], script);
    let mut services = LocalServices::with_engines(EngineRegistry::new().with(Arc::new(engine)));
    services.memory = Memories::plain(Arc::new(InMemoryStore::default()));
    services.memory_tool = false;
    services.routines = Routines::in_memory();
    services.plugins = services.plugins.with_secrets(Arc::new(InMemorySecretStore::default()));
    Server::start(services).expect("server")
}

fn next<T>(connection: &Connection, pick: impl Fn(ServerEvent) -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        match connection.try_recv() {
            Some(event) => {
                if let Some(found) = pick(event) {
                    return found;
                }
            }
            None => std::thread::yield_now(),
        }
    }
    panic!("nothing arrived");
}

fn hooked(server: &Server) -> (WebhookId, WebhookToken) {
    let owner = server.connect(Principal::LocalOwner, Arc::new(|| {}));
    let draft = RoutineDraft {
        name: "Orders".into(),
        brief: "Thank the buyer".into(),
        schedule: Schedule::Every { minutes: 60 },
        autonomy: Autonomy::Draft,
        model: None,
        connections: Vec::new(),
        answers: false,
    };
    let edits = vec![RoutineEdit::Create { draft }, RoutineEdit::Webhook { id: RoutineId(0), on: true }];
    let path = std::path::PathBuf::from("/repo");
    owner.send(Envelope::local(ClientCommand::EditRoutines { path: path.clone(), edits })).expect("send");
    owner.send(Envelope::local(ClientCommand::RevealWebhook { path, routine: RoutineId(0) })).expect("send");
    next(&owner, |event| match event {
        ServerEvent::RoutineWebhook { hook, token, .. } => Some((hook, token)),
        _ => None,
    })
}

async fn post(server: &Server, hook: &str, token: Option<&str>, body: Vec<u8>) -> StatusCode {
    let mut request = Request::post(format!("/hooks/{hook}"));
    if let Some(token) = token {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    let request = request.body(Body::from(body)).expect("request");
    superfik_server::hooks::routes(server.clone()).oneshot(request).await.expect("answer").status()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_call_with_the_token_is_taken_and_anything_else_is_not() {
    let server = server();
    let (hook, token) = hooked(&server);
    let hook = hook.to_string();
    assert_eq!(post(&server, &hook, Some(&token.0), b"{\"order\":42}".to_vec()).await, StatusCode::ACCEPTED);
    assert_eq!(post(&server, &hook, Some("guess"), b"{}".to_vec()).await, StatusCode::NOT_FOUND);
    assert_eq!(post(&server, &hook, None, b"{}".to_vec()).await, StatusCode::NOT_FOUND);
    assert_eq!(post(&server, "not-an-id", Some(&token.0), b"{}".to_vec()).await, StatusCode::NOT_FOUND);
    let big = vec![b'x'; MAX_WEBHOOK_BYTES + 1];
    assert_eq!(post(&server, &hook, Some(&token.0), big).await, StatusCode::PAYLOAD_TOO_LARGE);
}

fn with_channel(recorder: &superfik_channels::Recorder) -> (Server, superfik_core::ConnectionId) {
    use superfik_backend::{ChannelAccount, Channels, InMemoryChannelStore};
    let mut services = LocalServices::with_engines(EngineRegistry::new());
    let channels = Channels::new(
        vec![Arc::new(recorder.clone())],
        std::env::temp_dir().join("superfik-hook-tests"),
        Arc::new(InMemoryChannelStore::default()),
        Arc::new(superfik_backend::ChannelClock),
    );
    let connection = superfik_core::Connection {
        id: superfik_core::ConnectionId::new(),
        plugin: superfik_core::PluginId("whatsapp_cloud".into()),
        plugin_name: "WhatsApp Business".into(),
        label: "support".into(),
        icon: String::new(),
    };
    let account = ChannelAccount {
        connection: connection.clone(),
        kind: superfik_core::channel::ChannelKind::WhatsappCloud,
        credentials: superfik_channels::Credentials::default(),
    };
    channels.sync(&Principal::LocalOwner, &[account]);
    services.channels = channels;
    let deadline = Instant::now() + Duration::from_secs(5);
    while recorder.opened().is_empty() && Instant::now() < deadline {
        std::thread::yield_now();
    }
    (Server::start(services).expect("server"), connection.id)
}

async fn channel_call(server: &Server, request: Request<Body>) -> (StatusCode, String) {
    use http_body_util::BodyExt;
    let answer = superfik_server::hooks::routes(server.clone()).oneshot(request).await.expect("answer");
    let status = answer.status();
    let body = answer.into_body().collect().await.expect("body").to_bytes();
    (status, String::from_utf8_lossy(&body).into_owned())
}

#[tokio::test(flavor = "multi_thread")]
async fn a_providers_call_reaches_the_channel_account_and_its_query_and_headers_with_it() {
    let recorder = superfik_channels::Recorder::of(superfik_core::channel::ChannelKind::WhatsappCloud);
    let (server, connection) = with_channel(&recorder);
    let get = Request::get(format!("/hooks/channels/{connection}?hub.mode=subscribe&hub.challenge=42"))
        .body(Body::empty())
        .expect("request");
    let deadline = Instant::now() + Duration::from_secs(5);
    while channel_call(&server, Request::get(format!("/hooks/channels/{connection}")).body(Body::empty()).expect("r"))
        .await
        .0
        != StatusCode::OK
    {
        assert!(Instant::now() < deadline, "the link never opened");
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert_eq!(channel_call(&server, get).await.0, StatusCode::OK);
    let post = Request::post(format!("/hooks/channels/{connection}"))
        .header("X-Hub-Signature-256", "sha256=ab")
        .body(Body::from(r#"{"entry":[]}"#))
        .expect("request");
    assert_eq!(channel_call(&server, post).await.0, StatusCode::OK);

    let hooks = recorder.hooks();
    let challenge = hooks.iter().find(|h| h.query("hub.challenge").is_some()).expect("the GET");
    assert_eq!(challenge.query("hub.mode"), Some("subscribe"));
    let event = hooks.iter().find(|h| h.method == superfik_backend::HookMethod::Post).expect("the POST");
    assert_eq!(event.header("x-hub-signature-256"), Some("sha256=ab"));
    assert_eq!(event.body, br#"{"entry":[]}"#);
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unknown_channel_account_is_a_404() {
    let recorder = superfik_channels::Recorder::of(superfik_core::channel::ChannelKind::WhatsappCloud);
    let (server, _) = with_channel(&recorder);
    let other = superfik_core::ConnectionId::new();
    let call = |path: String| Request::post(path).body(Body::from("{}")).expect("request");
    assert_eq!(channel_call(&server, call(format!("/hooks/channels/{other}"))).await.0, StatusCode::NOT_FOUND);
    assert_eq!(channel_call(&server, call("/hooks/channels/nope".into())).await.0, StatusCode::NOT_FOUND);
}
