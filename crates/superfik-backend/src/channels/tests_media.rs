//! Files on channel accounts: kept when received, named in their message,
//! read by the agent, sent from the project and nothing else.

use std::sync::{Arc, Mutex};

use serde_json::json;
use superfik_channels::{Bytes, ChannelEvent, Recorder, Sent};
use superfik_core::channel::limits::{Ledger, Limits, Priority};
use superfik_core::channel::{
    Attachment, CHANNEL_READ_TOOL, CHANNEL_SEND_TOOL, Delivery, Media, MediaKind, Message, Source,
};
use superfik_core::{ConnectionId, Principal, RunId};

use super::super::mcp::ChannelTools;
use super::super::sender::{self, Outgoing, SystemClock};
use super::super::{Channels, InMemoryChannelStore, Inbound};

fn channels(root: &std::path::Path) -> Channels {
    Channels::new(Vec::new(), root.to_path_buf(), Arc::new(InMemoryChannelStore::default()), Arc::new(SystemClock))
}

fn pdf(name: &str) -> Media {
    Media {
        kind: MediaKind::Document,
        mime_type: Some("application/pdf".into()),
        filename: Some(name.into()),
        size: None,
        file: None,
    }
}

fn from_ana(id: &str, media: Media) -> Message {
    Message {
        id: id.into(),
        exchange: "5511999990000".into(),
        is_group: false,
        sender: Some("5511999990000".into()),
        sender_name: Some("Ana".into()),
        text: "segue a nota".into(),
        media: Some(media),
        mentioned_me: false,
        reply_to_me: false,
        at: 10,
        source: Source::Contact,
    }
}

fn report(channels: &Channels, connection: ConnectionId, event: ChannelEvent) {
    channels.observe(Inbound { owner: Principal::LocalOwner, connection, event });
}

#[test]
fn a_received_file_is_kept_in_the_accounts_folder_and_named_in_its_message() {
    let root = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
    let (channels, account) = (channels(root.path()), ConnectionId::new());
    let media = pdf("../../../.ssh/authorized_keys.pdf");
    let file = ChannelEvent::File { message: "m/1".into(), media: media.clone(), bytes: Bytes(b"%PDF".to_vec()) };
    report(&channels, account, file);
    let message = ChannelEvent::Message { message: from_ana("m/1", media), delivery: Delivery::Live };
    report(&channels, account, message);

    let book = channels.book(&Principal::LocalOwner, account);
    let kept = book.threads[0].messages[0].media.clone().and_then(|m| m.file).unwrap_or_else(|| panic!("not kept"));
    assert!(kept.starts_with(channels.media_dir(account)), "{}", kept.display());
    assert_eq!(std::fs::read(&kept).unwrap_or_default(), b"%PDF");
    assert_eq!(book.threads[0].messages[0].media.as_ref().and_then(|m| m.size), Some(4));
}

#[test]
fn a_file_too_large_is_named_but_not_kept() {
    let root = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
    let (channels, account) = (channels(root.path()), ConnectionId::new());
    let message = ChannelEvent::Message { message: from_ana("big", pdf("video.pdf")), delivery: Delivery::Live };
    report(&channels, account, message);
    let book = channels.book(&Principal::LocalOwner, account);
    assert_eq!(book.threads[0].messages[0].media.as_ref().and_then(|m| m.file.clone()), None);
}

fn call(tools: &ChannelTools, token: &str, name: &str, arguments: serde_json::Value) -> serde_json::Value {
    let message = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": name, "arguments": arguments } });
    tools.handle(token, &message).ok().flatten().unwrap_or_default()
}

fn answer(value: &serde_json::Value) -> (&str, bool) {
    let text = value["result"]["content"][0]["text"].as_str().unwrap_or_default();
    (text, value["result"]["isError"].as_bool().unwrap_or(false))
}

#[test]
fn an_agent_sees_where_a_file_was_kept_and_sends_only_project_files() {
    let (root, project) = (tempfile::tempdir(), tempfile::tempdir());
    let (root, project) = (root.unwrap_or_else(|e| panic!("{e}")), project.unwrap_or_else(|e| panic!("{e}")));
    std::fs::write(project.path().join("chart.png"), b"png").unwrap_or_else(|e| panic!("{e}"));
    let (channels, account) = (channels(root.path()), ConnectionId::new());
    report(
        &channels,
        account,
        ChannelEvent::File { message: "1".into(), media: pdf("nota.pdf"), bytes: Bytes(b"%PDF".to_vec()) },
    );
    report(
        &channels,
        account,
        ChannelEvent::Message { message: from_ana("1", pdf("nota.pdf")), delivery: Delivery::Live },
    );
    let published = Arc::new(Mutex::new(Vec::new()));
    let kept = Arc::clone(&published);
    let tools = ChannelTools::new(
        channels.clone(),
        Arc::new(move |_owner: &Principal, event| kept.lock().unwrap_or_else(|e| e.into_inner()).push(event)),
    );
    let grant = tools.grant(
        &Principal::LocalOwner,
        RunId::new(),
        project.path().to_path_buf(),
        vec![(account, "WhatsApp".into())],
        true,
    );

    let read = call(&tools, &grant.token, CHANNEL_READ_TOOL, json!({ "conversation": "5511999990000" }));
    assert!(answer(&read).0.contains("[document nota.pdf, saved at "), "{}", answer(&read).0);

    let sent = call(
        &tools,
        &grant.token,
        CHANNEL_SEND_TOOL,
        json!({ "conversation": "5511999990000", "text": "Aqui", "file": "chart.png" }),
    );
    assert!(!answer(&sent).1, "{}", answer(&sent).0);
    let drafts = channels.book(&Principal::LocalOwner, account).drafts;
    let attachment = drafts[0].attachment.clone().unwrap_or_else(|| panic!("no file"));
    assert_eq!(attachment.media.kind, MediaKind::Image);
    assert_eq!(attachment.media.size, Some(3));

    let received = channels.media_dir(account).join("1-nota.pdf");
    let forwarded = call(
        &tools,
        &grant.token,
        CHANNEL_SEND_TOOL,
        json!({ "conversation": "5511999990000", "file": received.to_string_lossy() }),
    );
    assert!(!answer(&forwarded).1, "a received file may go back out");

    for outside in ["/etc/hosts", "../outside.txt", "missing.png"] {
        let refused =
            call(&tools, &grant.token, CHANNEL_SEND_TOOL, json!({ "conversation": "5511999990000", "file": outside }));
        assert!(answer(&refused).1, "{outside} was taken");
    }
    assert_eq!(channels.book(&Principal::LocalOwner, account).drafts.len(), 2);
}

#[test]
fn the_sender_sends_a_file_with_its_caption_and_keeps_where_it_was() {
    let project = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
    let path = project.path().join("nota.pdf");
    std::fs::write(&path, b"%PDF-1.4").unwrap_or_else(|e| panic!("{e}"));
    let recorder = Recorder::default();
    let (queue, outgoing) = crossbeam_channel::unbounded();
    let attachment = Attachment::of(path.clone(), 8);
    let missing = Attachment::of(project.path().join("gone.pdf"), 8);
    for attachment in [missing, attachment] {
        let message = Outgoing {
            exchange: "5511999990000".into(),
            text: "Segue".into(),
            priority: Priority::Reply,
            attachment: Some(attachment),
        };
        queue.send(message).ok();
    }
    drop(queue);
    let recorded = Mutex::new(Vec::new());
    let record = |message: Message| recorded.lock().unwrap_or_else(|e| e.into_inner()).push(message);
    sender::run(&recorder, &InstantClock, Limits::default(), Ledger::default(), &outgoing, &record);

    let files: Vec<Sent> = recorder.sent().into_iter().filter(|s| matches!(s, Sent::File { .. })).collect();
    assert_eq!(
        files,
        [Sent::File {
            exchange: "5511999990000".into(),
            filename: Some("nota.pdf".into()),
            size: 8,
            caption: "Segue".into()
        }],
        "the missing file is skipped"
    );
    let recorded = recorded.into_inner().unwrap_or_else(|e| e.into_inner());
    assert_eq!(recorded[0].media.as_ref().and_then(|m| m.file.clone()), Some(path));
}

struct InstantClock;

impl sender::Clock for InstantClock {
    fn now_ms(&self) -> i64 {
        0
    }
    fn sleep(&self, _: std::time::Duration) {}
    fn draw(&self) -> f64 {
        0.0
    }
}
