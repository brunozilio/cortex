//! The Cloud API adapter against scripted Graph answers: no real number.

use std::sync::{Arc, Mutex};

use serde_json::json;
use superfik_core::ConnectionId;
use superfik_core::channel::{ChannelKind, Media, MediaKind};

use super::*;
use crate::http::{HttpResponse, Method, ScriptedHttp};
use crate::{Credentials, signature};

const GRAPH: &str = "https://graph.test/v21.0";
const SECRET: &str = "app-secret";

fn spec() -> LinkSpec {
    let values = [
        ("phone_number_id", "106540352242922"),
        ("access_token", "EAAG-token"),
        ("app_secret", SECRET),
        ("verify_token", "our-phrase"),
    ];
    LinkSpec {
        connection: ConnectionId::new(),
        kind: ChannelKind::WhatsappCloud,
        data_dir: std::env::temp_dir(),
        credentials: Credentials::new(values.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect()),
    }
}

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!("{}/tests/fixtures/whatsapp_cloud/{name}", env!("CARGO_MANIFEST_DIR"))).expect("fixture")
}

fn opened() -> (Arc<dyn Channel>, ScriptedHttp, Arc<Mutex<Vec<ChannelEvent>>>) {
    let http = ScriptedHttp::new([HttpResponse::ok_json(&json!({
        "display_phone_number": "+1 555-078-3881", "verified_name": "Superfik", "id": "106540352242922"
    }))]);
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&seen);
    let events: Events = Arc::new(move |event| sink.lock().expect("events").push(event));
    let channel = CloudOpener::at(Arc::new(http.clone()), GRAPH).open(&spec(), events).expect("open");
    (channel, http, seen)
}

fn signed(body: &[u8]) -> HookRequest {
    HookRequest::post(&[("X-Hub-Signature-256", &signature::meta_header(SECRET, body))], body.to_vec())
}

#[test]
fn linking_reads_the_number_with_the_token() {
    let (_, http, seen) = opened();
    let call = &http.calls()[0];
    assert_eq!(call.url, format!("{GRAPH}/106540352242922?fields=display_phone_number,verified_name"));
    assert!(call.headers.contains(&("Authorization".into(), "Bearer EAAG-token".into())));
    assert_eq!(seen.lock().expect("events")[0], ChannelEvent::Linked { account: "15550783881".into() });
}

#[test]
fn a_refused_token_fails_the_link_with_metas_reason() {
    let body = json!({ "error": { "message": "Invalid OAuth access token.", "code": 190 } });
    let http = ScriptedHttp::new([HttpResponse { status: 401, body: body.to_string().into_bytes() }]);
    let opened = CloudOpener::at(Arc::new(http), GRAPH).open(&spec(), Arc::new(|_| {}));
    assert!(matches!(opened, Err(ChannelError::Failed(why)) if why.contains("Invalid OAuth")));
}

#[test]
fn the_subscription_check_echoes_only_to_our_verify_token() {
    let (channel, _, _) = opened();
    let check = |token: &str| {
        channel.hook(&HookRequest::get(&[
            ("hub.mode", "subscribe"),
            ("hub.verify_token", token),
            ("hub.challenge", "77"),
        ]))
    };
    assert_eq!(check("our-phrase"), HookAnswer::Text("77".into()));
    assert_eq!(check("guess"), HookAnswer::Refused);
}

#[test]
fn a_signed_message_is_reported_and_an_unsigned_one_is_not() {
    let (channel, _, seen) = opened();
    let body = fixture("text.json");
    assert_eq!(channel.hook(&HookRequest::post(&[], body.clone())), HookAnswer::Refused);
    assert_eq!(seen.lock().expect("events").len(), 1, "only the link");
    assert_eq!(channel.hook(&signed(&body)), HookAnswer::Accepted);
    let events = seen.lock().expect("events");
    let ChannelEvent::Message { message, delivery } = &events[1] else { panic!("expected a message") };
    assert_eq!(message.text, "O checkout não abre");
    assert_eq!(*delivery, Delivery::Live);
}

#[test]
fn a_received_file_is_fetched_and_reported_before_its_message() {
    let (channel, http, seen) = opened();
    http.answer(HttpResponse::ok_json(&json!({
        "url": "https://lookaside.fbsbx.com/media/1037", "mime_type": "application/pdf", "file_size": 4, "id": "1037543291543636"
    })));
    http.answer(HttpResponse { status: 200, body: b"%PDF".to_vec() });
    assert_eq!(channel.hook(&signed(&fixture("document.json"))), HookAnswer::Accepted);
    let calls = http.calls();
    assert_eq!(calls[1].url, format!("{GRAPH}/1037543291543636"));
    assert_eq!(calls[2].url, "https://lookaside.fbsbx.com/media/1037");
    assert!(calls[2].headers.iter().any(|(k, _)| k == "Authorization"), "the file needs the token too");
    let events = seen.lock().expect("events");
    let ChannelEvent::File { media, bytes, .. } = &events[1] else { panic!("expected the file first") };
    assert_eq!(bytes.0, b"%PDF");
    assert_eq!(media.size, Some(4));
    assert!(matches!(&events[2], ChannelEvent::Message { message, .. } if message.media.is_some()));
}

#[test]
fn a_reply_goes_to_the_contacts_number_as_text() {
    let (channel, http, _) = opened();
    http.answer(HttpResponse::ok_json(&json!({ "messages": [{ "id": "wamid.sent" }] })));
    assert_eq!(channel.send_text("+55 11 99999-0000", "Olá, Ana!").expect("sent"), "wamid.sent");
    let call = &http.calls()[1];
    assert_eq!(call.url, format!("{GRAPH}/106540352242922/messages"));
    let body: serde_json::Value = serde_json::from_slice(call.body.as_deref().expect("body")).expect("json");
    assert_eq!(body["to"], "5511999990000");
    assert_eq!(body["type"], "text");
    assert_eq!(body["text"]["body"], "Olá, Ana!");
    assert!(channel.send_text("not a number", "x").is_err());
}

#[test]
fn a_bad_address_names_what_was_given() {
    let (channel, _, _) = opened();
    let error = channel.send_text("not a number", "x").expect_err("no digits");
    assert!(matches!(&error, ChannelError::Address(given) if given == "not a number"), "{error:?}");
}

#[test]
fn a_file_is_uploaded_then_sent_by_its_id_with_its_caption() {
    let (channel, http, _) = opened();
    http.answer(HttpResponse::ok_json(&json!({ "id": "media-9" })));
    http.answer(HttpResponse::ok_json(&json!({ "messages": [{ "id": "wamid.file" }] })));
    let media = Media {
        kind: MediaKind::Document,
        mime_type: Some("application/pdf".into()),
        filename: Some("cupom.pdf".into()),
        size: Some(4),
        file: None,
    };
    let file = OutgoingFile { media: &media, bytes: b"%PDF", caption: "Aqui está" };
    assert_eq!(channel.send_file("5511999990000", &file).expect("sent"), "wamid.file");
    let calls = http.calls();
    assert_eq!(calls[1].url, format!("{GRAPH}/106540352242922/media"));
    assert!(calls[1].text().contains("filename=\"cupom.pdf\""));
    assert!(calls[1].text().contains("%PDF"));
    let body: serde_json::Value = serde_json::from_slice(calls[2].body.as_deref().expect("body")).expect("json");
    assert_eq!(body["document"], json!({ "id": "media-9", "filename": "cupom.pdf", "caption": "Aqui está" }));
}

#[test]
fn typing_marks_the_contacts_last_message_read() {
    let (channel, http, _) = opened();
    channel.typing("5511999990000").expect("nothing to mark yet");
    assert_eq!(http.calls().len(), 1, "no call before the contact wrote");
    channel.hook(&signed(&fixture("text.json")));
    http.answer(HttpResponse::ok_json(&json!({ "success": true })));
    channel.typing("5511999990000").expect("typing");
    let call = http.calls().pop().expect("call");
    assert_eq!(call.method, Method::Post);
    let body: serde_json::Value = serde_json::from_slice(call.body.as_deref().expect("body")).expect("json");
    assert_eq!(body["status"], "read");
    assert_eq!(body["typing_indicator"]["type"], "text");
    assert_eq!(body["message_id"], "wamid.HBgLMTY1MDM4Nzk0MzkVAgASGBQzQTRBNjU5OUFFRTAzODEwMTQ0RgA=");
}
