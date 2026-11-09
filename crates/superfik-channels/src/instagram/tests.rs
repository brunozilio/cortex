//! The Instagram adapter against scripted Graph answers: no real account.

use std::sync::{Arc, Mutex};

use serde_json::json;
use superfik_core::ConnectionId;
use superfik_core::channel::{ChannelKind, Source};

use super::*;
use crate::http::{HttpResponse, ScriptedHttp};
use crate::{Credentials, signature};

const GRAPH: &str = "https://graph.test/v21.0";
const SECRET: &str = "app-secret";

fn spec() -> LinkSpec {
    let values = [
        ("page_id", "1045600000000"),
        ("page_access_token", "EAAP-token"),
        ("app_secret", SECRET),
        ("verify_token", "our-phrase"),
    ];
    LinkSpec {
        connection: ConnectionId::new(),
        kind: ChannelKind::Instagram,
        data_dir: std::env::temp_dir(),
        credentials: Credentials::new(values.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect()),
    }
}

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!("{}/tests/fixtures/instagram/{name}", env!("CARGO_MANIFEST_DIR"))).expect("fixture")
}

fn signed(body: &[u8]) -> HookRequest {
    HookRequest::post(&[("X-Hub-Signature-256", &signature::meta_header(SECRET, body))], body.to_vec())
}

type Seen = Arc<Mutex<Vec<ChannelEvent>>>;

fn opened() -> (Arc<dyn Channel>, ScriptedHttp, Seen) {
    let http = ScriptedHttp::new([HttpResponse::ok_json(&json!({
        "instagram_business_account": { "id": "17841405822304914", "username": "superfik.app" }, "id": "1045600000000"
    }))]);
    let seen: Seen = Arc::default();
    let sink = Arc::clone(&seen);
    let events: Events = Arc::new(move |event| sink.lock().expect("events").push(event));
    let channel = InstagramOpener::at(Arc::new(http.clone()), GRAPH).open(&spec(), events).expect("open");
    (channel, http, seen)
}

fn body_of(http: &ScriptedHttp, call: usize) -> serde_json::Value {
    serde_json::from_slice(http.calls()[call].body.as_deref().expect("body")).expect("json")
}

#[test]
fn linking_finds_the_instagram_account_of_the_page() {
    let (_, http, seen) = opened();
    assert!(http.calls()[0].url.starts_with(&format!("{GRAPH}/1045600000000?fields=instagram_business_account")));
    assert_eq!(seen.lock().expect("events")[0], ChannelEvent::Linked { account: "@superfik.app".into() });
}

#[test]
fn a_page_without_instagram_fails_plainly() {
    let http = ScriptedHttp::new([HttpResponse::ok_json(&json!({ "id": "1045600000000" }))]);
    let opened = InstagramOpener::at(Arc::new(http), GRAPH).open(&spec(), Arc::new(|_| {}));
    assert!(matches!(opened, Err(ChannelError::Failed(why)) if why.contains("no professional Instagram")));
}

#[test]
fn a_signed_dm_is_reported_with_the_contacts_name() {
    let (channel, http, seen) = opened();
    assert_eq!(channel.hook(&HookRequest::post(&[], fixture("message.json"))), HookAnswer::Refused, "unsigned");
    http.answer(HttpResponse::ok_json(&json!({ "name": "Ana Lima", "username": "ana.lima" })));
    assert_eq!(channel.hook(&signed(&fixture("message.json"))), HookAnswer::Accepted);
    channel.hook(&signed(&fixture("message.json")));
    assert_eq!(http.calls().iter().filter(|c| c.url.contains("fields=name,username")).count(), 1);
    let events = seen.lock().expect("events");
    let ChannelEvent::Message { message, .. } = &events[1] else { panic!("expected a message") };
    assert_eq!(message.sender_name.as_deref(), Some("Ana Lima"));
}

#[test]
fn an_attachment_is_downloaded_before_its_message() {
    let (channel, http, seen) = opened();
    http.answer(HttpResponse::ok_json(&json!({ "username": "ana.lima" })));
    http.answer(HttpResponse { status: 200, body: b"\x89PNG".to_vec() });
    channel.hook(&signed(&fixture("image.json")));
    let calls = http.calls();
    assert_eq!(calls[2].url, "https://lookaside.fbsbx.com/ig_messaging_cdn/1");
    let events = seen.lock().expect("events");
    assert!(matches!(&events[1], ChannelEvent::File { bytes, .. } if bytes.0 == b"\x89PNG"));
    assert!(
        matches!(&events[2], ChannelEvent::Message { message, .. } if message.sender_name.as_deref() == Some("@ana.lima"))
    );
}

#[test]
fn an_attachment_off_metas_cdn_is_not_fetched_but_its_message_arrives() {
    let (channel, http, seen) = opened();
    http.answer(HttpResponse::ok_json(&json!({ "username": "ana.lima" })));
    let body = String::from_utf8(fixture("image.json"))
        .expect("text")
        .replace("https://lookaside.fbsbx.com/ig_messaging_cdn/1", "http://169.254.169.254/latest/meta-data");
    channel.hook(&signed(body.as_bytes()));
    assert!(http.calls().iter().all(|call| !call.url.contains("169.254")), "{:?}", http.calls());
    let events = seen.lock().expect("events");
    assert!(events.iter().all(|event| !matches!(event, ChannelEvent::File { .. })));
    assert!(events.iter().any(|event| matches!(event, ChannelEvent::Message { .. })));
}

#[test]
fn a_dm_reply_goes_to_the_page_and_its_echo_is_the_agents() {
    let (channel, http, seen) = opened();
    http.answer(HttpResponse::ok_json(
        &json!({ "recipient_id": "6857162164316212", "message_id": "aWdfZAG1faXRlbTpJR01lc3NhZA" }),
    ));
    assert_eq!(channel.send_text("6857162164316212", "Entregamos sim!").expect("sent"), "aWdfZAG1faXRlbTpJR01lc3NhZA");
    assert_eq!(http.calls()[1].url, format!("{GRAPH}/1045600000000/messages"));
    assert_eq!(
        body_of(&http, 1),
        json!({ "recipient": { "id": "6857162164316212" }, "message": { "text": "Entregamos sim!" } })
    );
    channel.hook(&signed(&fixture("echo.json")));
    let events = seen.lock().expect("events");
    assert!(matches!(&events[1], ChannelEvent::Message { message, .. } if message.source == Source::Agent));
}

#[test]
fn a_comment_is_answered_under_it() {
    let (channel, http, _) = opened();
    http.answer(HttpResponse::ok_json(&json!({ "id": "17865799348089041" })));
    channel.send_text("comment:17865799348089039", "Te mandei por DM!").expect("replied");
    assert_eq!(http.calls()[1].url, format!("{GRAPH}/17865799348089039/replies"));
    assert_eq!(body_of(&http, 1), json!({ "message": "Te mandei por DM!" }));
    channel.typing("comment:17865799348089039").expect("nothing to show");
    assert_eq!(http.calls().len(), 2);
    assert!(channel.send_text("ana.lima", "x").is_err(), "not an Instagram-scoped id");
}

#[test]
fn typing_shows_in_a_dm() {
    let (channel, http, _) = opened();
    http.answer(HttpResponse::ok_json(&json!({ "recipient_id": "6857162164316212" })));
    channel.typing("6857162164316212").expect("typing");
    assert_eq!(body_of(&http, 1)["sender_action"], "typing_on");
}
