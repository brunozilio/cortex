use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_channel::Receiver;
use superfik_channels::{ChannelEvent, Recorder, Sent};
use superfik_core::channel::{ChannelKind, Delivery, LinkStatus, Message, Source};
use superfik_core::{Connection, ConnectionId, PluginId, Principal, RunId};
use superfik_protocol::ServerEvent;

use super::sender::Clock;
use super::*;

pub(super) struct Instant0;

impl Clock for Instant0 {
    fn now_ms(&self) -> i64 {
        now_ms()
    }
    fn sleep(&self, _: Duration) {}
    fn draw(&self) -> f64 {
        0.0
    }
}

pub(super) fn account() -> Connection {
    Connection {
        id: ConnectionId::new(),
        plugin: PluginId("whatsapp".into()),
        plugin_name: "WhatsApp".into(),
        label: "support".into(),
        icon: String::new(),
    }
}

pub(super) fn linking(account: &Connection, kind: ChannelKind) -> ChannelAccount {
    ChannelAccount { connection: account.clone(), kind, credentials: superfik_channels::Credentials::default() }
}

fn linked() -> (Channels, Recorder, Receiver<Inbound>, Connection) {
    let recorder = Recorder::default();
    let channels = Channels::new(
        vec![Arc::new(recorder.clone())],
        std::env::temp_dir().join("superfik-channel-tests"),
        Arc::new(InMemoryChannelStore::default()),
        Arc::new(Instant0),
    );
    let (inbound, reports) = crossbeam_channel::unbounded();
    channels.set_inbound(inbound);
    let account = account();
    channels.sync(&Principal::LocalOwner, &[linking(&account, ChannelKind::WhatsappWeb)]);
    wait(|| !recorder.opened().is_empty());
    wait(|| channels.send(&Principal::LocalOwner, account.id, "x", "", None).is_ok());
    while !matches!(next_report(&reports).event, ChannelEvent::Message { .. }) {}
    (channels, recorder, reports, account)
}

fn next_report(reports: &Receiver<Inbound>) -> Inbound {
    reports.recv_timeout(Duration::from_secs(60)).unwrap_or_else(|_| panic!("no report"))
}

pub(super) fn wait(done: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !done() {
        assert!(Instant::now() < deadline, "timed out");
        std::thread::sleep(Duration::from_millis(5));
    }
}

pub(super) fn message(id: &str, source: Source, at: i64) -> Message {
    Message {
        id: id.into(),
        exchange: "5511999990000".into(),
        is_group: false,
        sender: Some("5511999990000".into()),
        sender_name: Some("Ana".into()),
        text: "O checkout não abre".into(),
        media: None,
        mentioned_me: false,
        reply_to_me: false,
        at,
        source,
    }
}

pub(super) fn report(account: &Connection, event: ChannelEvent) -> Inbound {
    Inbound { owner: Principal::LocalOwner, connection: account.id, event }
}

#[test]
fn linking_reports_the_qr_and_then_the_number() {
    let (channels, recorder, reports, account) = linked();
    recorder.play(ChannelEvent::Qr { code: "2@abc".into(), valid_secs: 60 });
    let qr = reports.try_recv().unwrap_or_else(|_| panic!("no report"));
    assert!(reports.is_empty(), "nothing else was reported");
    let observed = channels.observe(qr);
    assert!(matches!(
        observed.update,
        Some(ServerEvent::Channel { status: LinkStatus::Pairing { ref qr }, .. }) if qr == "2@abc"
    ));
    let linked = channels.observe(report(&account, ChannelEvent::Linked { account: "5511888880000".into() }));
    assert!(matches!(linked.update, Some(ServerEvent::Channel { status: LinkStatus::Linked { .. }, .. })));
}

#[test]
fn a_live_contact_message_asks_for_an_answer_once() {
    let (channels, _, _, account) = linked();
    let live = |m: Message| report(&account, ChannelEvent::Message { message: m, delivery: Delivery::Live });
    let observed = channels.observe(live(message("1", Source::Contact, now_ms())));
    let incoming = observed.incoming.unwrap_or_else(|| panic!("no incoming"));
    assert_eq!(incoming.exchange, "5511999990000");
    assert_eq!(incoming.from.as_deref(), Some("Ana"));
    assert_eq!(incoming.channel, "WhatsApp · support");
    assert!(channels.observe(live(message("1", Source::Contact, now_ms()))).incoming.is_none(), "seen before");
    assert!(channels.observe(live(message("2", Source::Agent, now_ms()))).incoming.is_none());
    assert_eq!(channels.book(&Principal::LocalOwner, account.id).recent("5511999990000", 10).len(), 2);
}

#[test]
fn history_delivered_after_linking_wakes_nobody() {
    let (channels, _, _, account) = linked();
    let old = ChannelEvent::Message { message: message("1", Source::Contact, 1_000), delivery: Delivery::Backlog };
    assert!(channels.observe(report(&account, old)).incoming.is_none());
}

#[test]
fn drafts_wait_and_an_approved_one_goes_out_typed() {
    let (channels, recorder, _, account) = linked();
    let event = channels.draft(&Principal::LocalOwner, account.id, "5511999990000", "Olá, Ana!", None, RunId::new());
    let ServerEvent::Channel { drafts, .. } = event else { panic!("expected the account") };
    assert_eq!(drafts.len(), 1);
    assert!(!recorder.sent().iter().any(|s| matches!(s, Sent::Text { text, .. } if text == "Olá, Ana!")));

    let settled = channels.settle_draft(&Principal::LocalOwner, account.id, drafts[0].id, Some("Olá, Ana! 😊"));
    assert!(matches!(settled, Ok(ServerEvent::Channel { ref drafts, .. }) if drafts.is_empty()));
    wait(|| recorder.sent().iter().any(|s| matches!(s, Sent::Text { text, .. } if text == "Olá, Ana! 😊")));
    assert!(channels.settle_draft(&Principal::LocalOwner, account.id, drafts[0].id, None).is_err(), "gone");
}

#[test]
fn removing_an_account_unlinks_its_device() {
    let (channels, recorder, _, account) = linked();
    channels.sync(&Principal::LocalOwner, &[]);
    assert!(recorder.sent().contains(&Sent::Unlinked));
    assert!(channels.send(&Principal::LocalOwner, account.id, "x", "hi", None).is_err());
    assert!(channels.list(&Principal::LocalOwner).is_empty());
}

#[test]
fn an_account_of_a_kind_nobody_opens_fails_plainly() {
    let channels = Channels::none();
    let (inbound, reports) = crossbeam_channel::unbounded();
    channels.set_inbound(inbound);
    let account = account();
    channels.sync(&Principal::LocalOwner, &[linking(&account, ChannelKind::Email)]);
    let failed = channels.observe(next_report(&reports));
    assert!(matches!(failed.update, Some(ServerEvent::Channel { status: LinkStatus::Failed { .. }, .. })));
}
