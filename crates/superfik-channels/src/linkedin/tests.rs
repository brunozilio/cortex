//! The LinkedIn adapter against scripted answers: no real page.

use std::sync::{Arc, Mutex};

use hmac::Mac;
use serde_json::json;
use superfik_core::ConnectionId;
use superfik_core::channel::{Message, Source};

use super::*;
use crate::Credentials;
use crate::http::{HttpResponse, ScriptedHttp};

const API: &str = "https://api.linkedin.test/rest";
const THREAD: &str = "comment:urn:li:comment:(urn:li:activity:7250000000000000001,7250000000000000100)";

fn spec(secret: bool) -> LinkSpec {
    let mut values = vec![("organization_id", "2414183"), ("access_token", "AQX-token")];
    if secret {
        values.push(("client_secret", "client-secret"));
    }
    LinkSpec {
        connection: ConnectionId::new(),
        kind: ChannelKind::Linkedin,
        data_dir: std::env::temp_dir(),
        credentials: Credentials::new(values.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect()),
    }
}

fn fixture(name: &str) -> serde_json::Value {
    let path = format!("{}/tests/fixtures/linkedin/{name}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_slice(&std::fs::read(path).expect("fixture")).expect("json")
}

type Seen = Arc<Mutex<Vec<ChannelEvent>>>;

fn linked(secret: bool) -> (Arc<LinkedinChannel>, ScriptedHttp, Seen) {
    let http = ScriptedHttp::new([HttpResponse::ok_json(&json!({ "id": 2414183, "localizedName": "Superfik" }))]);
    let seen: Seen = Arc::default();
    let sink = Arc::clone(&seen);
    let events: Events = Arc::new(move |event| sink.lock().expect("events").push(event));
    let channel = LinkedinOpener::at(Arc::new(http.clone()), API).link(&spec(secret), events).expect("link");
    (channel, http, seen)
}

fn script_read(http: &ScriptedHttp, comments: serde_json::Value) {
    http.answer(HttpResponse::ok_json(&fixture("posts.json")));
    http.answer(HttpResponse::ok_json(&comments));
    http.answer(HttpResponse::ok_json(&json!({ "elements": [] })));
}

fn messages(seen: &Seen) -> Vec<(Message, Delivery)> {
    let events = seen.lock().expect("events");
    events
        .iter()
        .filter_map(|e| match e {
            ChannelEvent::Message { message, delivery } => Some((message.clone(), *delivery)),
            _ => None,
        })
        .collect()
}

#[test]
fn linking_reads_the_organizations_name_with_the_versioned_headers() {
    let (_, http, seen) = linked(false);
    let call = &http.calls()[0];
    assert_eq!(call.url, format!("{API}/organizations/2414183"));
    assert!(call.headers.contains(&("LinkedIn-Version".into(), VERSION.into())));
    assert!(call.headers.contains(&("X-Restli-Protocol-Version".into(), "2.0.0".into())));
    assert_eq!(seen.lock().expect("events")[0], ChannelEvent::Linked { account: "Superfik".into() });
}

#[test]
fn comments_are_read_once_history_first_then_news() {
    let (channel, http, seen) = linked(false);
    script_read(&http, fixture("comments.json"));
    channel.poll();
    let calls = http.calls();
    assert_eq!(calls[1].url, format!("{API}/posts?q=author&author=urn%3Ali%3Aorganization%3A2414183&count=10"));
    assert_eq!(calls[2].url, format!("{API}/socialActions/urn%3Ali%3Ashare%3A7250000000000000000/comments"));
    let first = messages(&seen);
    assert_eq!(first.len(), 2);
    assert!(first.iter().all(|(_, d)| *d == Delivery::Backlog));

    let mut more = fixture("comments.json");
    more["elements"].as_array_mut().expect("elements").push(json!({
        "$URN": "urn:li:comment:(urn:li:activity:7250000000000000001,7250000000000000102)",
        "actor": "urn:li:person:Zz9", "object": "urn:li:share:7250000000000000000",
        "message": { "text": "And Windows?" }, "created": { "time": 1_793_016_120_000_i64 }
    }));
    script_read(&http, more);
    channel.poll();
    let all = messages(&seen);
    assert_eq!(all.len(), 3, "only the new comment");
    assert_eq!(all[2].1, Delivery::Live);
    assert_eq!(all[2].0.source, Source::Contact);
}

#[test]
fn a_comment_is_answered_under_it_on_its_post() {
    let (channel, http, _) = linked(false);
    assert!(channel.send_text(THREAD, "x").is_err(), "its post is not known before a read");
    script_read(&http, fixture("comments.json"));
    channel.poll();
    http.answer(HttpResponse::ok_json(&json!({ "$URN": "urn:li:comment:(urn:li:activity:1,2)" })));
    assert_eq!(channel.send_text(THREAD, "Yes, soon!").expect("replied"), "urn:li:comment:(urn:li:activity:1,2)");
    let call = http.calls().pop().expect("call");
    assert_eq!(call.url, format!("{API}/socialActions/urn%3Ali%3Ashare%3A7250000000000000000/comments"));
    let body: serde_json::Value = serde_json::from_slice(call.body.as_deref().expect("body")).expect("json");
    assert_eq!(body["actor"], "urn:li:organization:2414183");
    assert_eq!(body["parentComment"], "urn:li:comment:(urn:li:activity:7250000000000000001,7250000000000000100)");
    assert_eq!(body["message"]["text"], "Yes, soon!");
}

#[test]
fn the_feed_publishes_a_post_as_the_page() {
    let (channel, http, _) = linked(false);
    http.answer(HttpResponse { status: 201, body: Vec::new() });
    channel.send_text(FEED, "Superfik 1.1 ships today.").expect("posted");
    let call = http.calls().pop().expect("call");
    assert_eq!(call.url, format!("{API}/posts"));
    let body: serde_json::Value = serde_json::from_slice(call.body.as_deref().expect("body")).expect("json");
    assert_eq!(body["author"], "urn:li:organization:2414183");
    assert_eq!(body["commentary"], "Superfik 1.1 ships today.");
    assert!(channel.send_text("urn:li:person:Ab1Cd2", "x").is_err(), "members' messages are closed");
}

#[test]
fn a_signed_notification_reads_the_page_and_the_challenge_is_answered() {
    let (channel, http, seen) = linked(true);
    let challenge = channel.hook(&HookRequest::get(&[("challengeCode", "890e4665")]));
    assert!(matches!(challenge, HookAnswer::Json(json) if json.contains("challengeResponse")));
    let body = br#"{"type":"ORGANIZATION_SOCIAL_ACTION_NOTIFICATIONS"}"#;
    assert_eq!(channel.hook(&HookRequest::post(&[("X-LI-Signature", "00")], body.to_vec())), HookAnswer::Refused);
    let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(b"client-secret").expect("key");
    mac.update(b"hmacsha256=");
    mac.update(body);
    let signature = hex::encode(mac.finalize().into_bytes());
    script_read(&http, fixture("comments.json"));
    assert_eq!(
        channel.hook(&HookRequest::post(&[("X-LI-Signature", &signature)], body.to_vec())),
        HookAnswer::Accepted
    );
    assert_eq!(messages(&seen).len(), 2, "read at once");
    let (unsigned, _, _) = linked(false);
    assert_eq!(unsigned.hook(&HookRequest::get(&[("challengeCode", "1")])), HookAnswer::Refused, "no client secret");
}
