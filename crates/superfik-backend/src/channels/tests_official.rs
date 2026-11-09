//! Accounts on the providers' official APIs: their webhook reaches the
//! link, the account shows where it is, and Meta's 24-hour window holds.

use std::sync::Arc;

use superfik_channels::{ChannelEvent, HookAnswer, HookRequest, Recorder, Sent};
use superfik_core::channel::{ChannelKind, Delivery, LinkStatus, Source};
use superfik_core::{Connection, ConnectionId, Principal};
use superfik_protocol::ServerEvent;

use super::tests::{Instant0, account, linking, message, report, wait};
use super::*;

const HOUR_MS: i64 = 60 * 60 * 1_000;

fn linked_as(kind: ChannelKind) -> (Channels, Recorder, Connection) {
    let recorder = Recorder::of(kind);
    let channels = Channels::new(
        vec![Arc::new(recorder.clone())],
        std::env::temp_dir().join("superfik-channel-tests"),
        Arc::new(InMemoryChannelStore::default()),
        Arc::new(Instant0),
    );
    let (inbound, _reports) = crossbeam_channel::unbounded();
    channels.set_inbound(inbound);
    let account = account();
    channels.sync(&Principal::LocalOwner, &[linking(&account, kind)]);
    let ping = HookRequest::get(&[]);
    wait(|| channels.hook(account.id, &ping) == HookAnswer::Accepted);
    (channels, recorder, account)
}

#[test]
fn a_providers_call_reaches_its_accounts_link_and_no_other() {
    let (channels, recorder, account) = linked_as(ChannelKind::WhatsappCloud);
    let call = HookRequest::post(&[("X-Hub-Signature-256", "sha256=00")], br#"{"entry":[]}"#.to_vec());
    assert_eq!(channels.hook(account.id, &call), HookAnswer::Accepted);
    assert!(recorder.hooks().contains(&call));
    assert_eq!(channels.hook(ConnectionId::new(), &call), HookAnswer::Refused, "unknown account");
}

#[test]
fn a_linked_official_account_shows_its_webhooks_path() {
    let (channels, _, account) = linked_as(ChannelKind::Instagram);
    let linked = channels.observe(report(&account, ChannelEvent::Linked { account: "superfik.app".into() }));
    let Some(ServerEvent::Channel { status: LinkStatus::Linked { hook, .. }, .. }) = linked.update else {
        panic!("expected the linked account")
    };
    assert_eq!(hook, Some(format!("/hooks/channels/{}", account.id)));
}

#[test]
fn meta_accounts_answer_only_inside_the_24_hour_window() {
    let (channels, recorder, account) = linked_as(ChannelKind::WhatsappCloud);
    let owner = Principal::LocalOwner;
    let refused = channels.send(&owner, account.id, "5511999990000", "Oi!", None);
    assert!(refused.is_err_and(|why| why.contains("24 hours")), "the person never wrote");

    let old = message("old", Source::Contact, now_ms() - 25 * HOUR_MS);
    channels.observe(report(&account, ChannelEvent::Message { message: old, delivery: Delivery::Live }));
    assert!(channels.send(&owner, account.id, "5511999990000", "Oi!", None).is_err(), "a day ago");

    let fresh = message("fresh", Source::Contact, now_ms() - HOUR_MS);
    channels.observe(report(&account, ChannelEvent::Message { message: fresh, delivery: Delivery::Live }));
    assert_eq!(channels.send(&owner, account.id, "5511999990000", "Oi!", None), Ok(()));
    wait(|| recorder.sent().iter().any(|s| matches!(s, Sent::Text { text, .. } if text == "Oi!")));
}

#[test]
fn channels_without_a_window_start_conversations() {
    let (channels, _, account) = linked_as(ChannelKind::X);
    assert_eq!(channels.send(&Principal::LocalOwner, account.id, "dm:42", "Hi!", None), Ok(()));
}
