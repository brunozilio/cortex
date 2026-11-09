//! The X adapter against scripted answers: no real account.

use std::sync::{Arc, Mutex};

use base64::Engine;
use hmac::Mac;
use serde_json::json;
use superfik_core::ConnectionId;
use superfik_core::channel::ChannelKind;

use super::*;
use crate::Credentials;
use crate::http::{HttpResponse, ScriptedHttp};

const API: &str = "https://api.x.test";
const ME: &str = "1580000000000000001";

fn spec() -> LinkSpec {
    let values = [
        ("api_key", "consumer-key"),
        ("api_secret", "consumer-secret"),
        ("access_token", "1580000000000000001-token"),
        ("access_token_secret", "token-secret"),
    ];
    LinkSpec {
        connection: ConnectionId::new(),
        kind: ChannelKind::X,
        data_dir: std::env::temp_dir(),
        credentials: Credentials::new(values.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect()),
    }
}

fn fixture(name: &str) -> serde_json::Value {
    let path = format!("{}/tests/fixtures/x/{name}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_slice(&std::fs::read(path).expect("fixture")).expect("json")
}

type Seen = Arc<Mutex<Vec<ChannelEvent>>>;

fn linked() -> (Arc<XChannel>, ScriptedHttp, Seen) {
    let http = ScriptedHttp::new([HttpResponse::ok_json(&json!({ "data": { "id": ME, "username": "superfikapp" } }))]);
    let seen: Seen = Arc::default();
    let sink = Arc::clone(&seen);
    let events: Events = Arc::new(move |event| sink.lock().expect("events").push(event));
    let stamp: Stamp = Arc::new(|| ("nonce".into(), 1_793_016_000));
    let channel = XOpener::at(Arc::new(http.clone()), API, stamp).link(&spec(), events).expect("link");
    (channel, http, seen)
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
fn linking_reads_the_user_with_a_signed_call() {
    let (_, http, seen) = linked();
    let call = &http.calls()[0];
    assert_eq!(call.url, format!("{API}/2/users/me"));
    let (_, auth) = call.headers.iter().find(|(k, _)| k == "Authorization").expect("signed");
    assert!(auth.starts_with("OAuth oauth_consumer_key=\"consumer-key\""));
    assert!(auth.contains("oauth_token=\"1580000000000000001-token\"") && auth.contains("oauth_signature="));
    assert_eq!(seen.lock().expect("events")[0], ChannelEvent::Linked { account: "@superfikapp".into() });
}

#[test]
fn the_first_read_is_history_and_the_next_is_news() {
    let (channel, http, seen) = linked();
    http.answer(HttpResponse::ok_json(&fixture("mentions.json")));
    http.answer(HttpResponse::ok_json(&fixture("dm_events.json")));
    channel.poll();
    let first = messages(&seen);
    assert_eq!(first.len(), 3, "one mention and two DMs");
    assert!(first.iter().all(|(_, d)| *d == Delivery::Backlog));
    assert_eq!(first[1].0.text, "Não consigo entrar na minha conta", "oldest DM first");

    let newer = json!({ "data": [{ "id": "1850000000000000104", "event_type": "MessageCreate", "text": "Alô?",
        "sender_id": "2244994945", "dm_conversation_id": "1580000000000000001-2244994945",
        "created_at": "2026-10-26T12:10:00.000Z" }] });
    http.answer(HttpResponse::ok_json(&json!({ "meta": { "result_count": 0 } })));
    http.answer(HttpResponse::ok_json(&json!({ "data": newer["data"].as_array().expect("events").iter()
        .cloned().chain(fixture("dm_events.json")["data"].as_array().expect("events").iter().cloned())
        .collect::<Vec<_>>() })));
    channel.poll();
    let all = messages(&seen);
    assert_eq!(all.len(), 4, "only the new DM");
    assert_eq!(all[3].1, Delivery::Live);
    let calls = http.calls();
    assert!(calls[3].url.contains("since_id=1850000000000000011"), "mentions since the newest seen");
}

#[test]
fn a_thread_is_answered_under_the_contacts_latest_post() {
    let (channel, http, _) = linked();
    http.answer(HttpResponse::ok_json(&fixture("mentions.json")));
    http.answer(HttpResponse::ok_json(&json!({ "meta": { "result_count": 0 } })));
    channel.poll();
    http.answer(HttpResponse::ok_json(&json!({ "data": { "id": "1850000000000000020", "text": "…" } })));
    let id = channel.send_text("post:1850000000000000000", "Já corrigimos, Ana!").expect("posted");
    assert_eq!(id, "1850000000000000020");
    let call = http.calls().pop().expect("call");
    assert_eq!(call.url, format!("{API}/2/tweets"));
    let body: serde_json::Value = serde_json::from_slice(call.body.as_deref().expect("body")).expect("json");
    assert_eq!(body["reply"]["in_reply_to_tweet_id"], "1850000000000000010");
}

#[test]
fn a_dm_goes_to_the_person_and_its_echo_is_the_agents() {
    let (channel, http, seen) = linked();
    http.answer(HttpResponse::ok_json(
        &json!({ "data": { "dm_conversation_id": "x", "dm_event_id": "1850000000000000102" } }),
    ));
    channel.send_text("dm:2244994945", "Oi Ana! Pode me mandar o e-mail da conta?").expect("sent");
    assert_eq!(http.calls()[1].url, format!("{API}/2/dm_conversations/with/2244994945/messages"));
    http.answer(HttpResponse::ok_json(&json!({ "meta": { "result_count": 0 } })));
    http.answer(HttpResponse::ok_json(&fixture("dm_events.json")));
    channel.poll();
    let echo = messages(&seen).into_iter().find(|(m, _)| m.id == "1850000000000000102").expect("echo");
    assert_eq!(echo.0.source, Source::Agent);
    assert!(channel.send_text("analima", "x").is_err());
}

#[test]
fn account_activity_answers_the_crc_and_takes_only_signed_events() {
    let (channel, _, seen) = linked();
    let crc = channel.hook(&HookRequest::get(&[("crc_token", "challenge")]));
    assert!(matches!(crc, HookAnswer::Json(json) if json.contains("response_token")));
    let body = std::fs::read(format!("{}/tests/fixtures/x/activity.json", env!("CARGO_MANIFEST_DIR"))).expect("f");
    let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(b"consumer-secret").expect("key");
    mac.update(&body);
    let signature = format!("sha256={}", base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes()));
    assert_eq!(channel.hook(&HookRequest::post(&[], body.clone())), HookAnswer::Refused);
    let signed = HookRequest::post(&[("x-twitter-webhooks-signature", &signature)], body);
    assert_eq!(channel.hook(&signed), HookAnswer::Accepted);
    let found = messages(&seen);
    assert_eq!(found.len(), 2);
    assert!(found.iter().all(|(_, d)| *d == Delivery::Live));
}

#[test]
fn what_the_webhook_brought_is_not_reported_again_by_the_next_read() {
    let (channel, http, seen) = linked();
    http.answer(HttpResponse::ok_json(&fixture("mentions.json")));
    http.answer(HttpResponse::ok_json(&fixture("dm_events.json")));
    channel.poll();
    let body = std::fs::read(format!("{}/tests/fixtures/x/activity.json", env!("CARGO_MANIFEST_DIR"))).expect("f");
    let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(b"consumer-secret").expect("key");
    mac.update(&body);
    let signature = format!("sha256={}", base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes()));
    let signed = HookRequest::post(&[("x-twitter-webhooks-signature", &signature)], body);
    assert_eq!(channel.hook(&signed), HookAnswer::Accepted);
    assert_eq!(messages(&seen).len(), 5, "three read, two pushed");

    http.answer(HttpResponse::ok_json(&json!({
        "data": [{ "id": "1850000000000000012", "author_id": "2244994945",
            "conversation_id": "1850000000000000000", "created_at": "2026-10-26T12:02:00.000Z",
            "text": "@superfikapp alguma previsão?" }],
        "includes": { "users": [{ "id": "2244994945", "name": "Ana Lima", "username": "analima" }] },
        "meta": { "newest_id": "1850000000000000012", "result_count": 1 }
    })));
    let pushed = json!({ "id": "1850000000000000103", "event_type": "MessageCreate", "text": "Mandei o e-mail por aqui",
        "sender_id": "2244994945", "dm_conversation_id": "1580000000000000001-2244994945",
        "created_at": "2026-10-26T12:06:00.000Z" });
    let mut events = fixture("dm_events.json");
    events["data"].as_array_mut().expect("events").insert(0, pushed);
    http.answer(HttpResponse::ok_json(&events));
    channel.poll();
    assert_eq!(messages(&seen).len(), 5, "the webhook's mention and DM are not news twice");
}
