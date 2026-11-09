use super::*;

const AT: i64 = 1_793_016_000_000;
const MINUTE: i64 = 60_000;

fn message(source: Source, at: i64) -> Message {
    Message {
        id: format!("m{at}"),
        exchange: "5511999990000".into(),
        is_group: false,
        sender: Some("5511999990000".into()),
        sender_name: Some("Ana".into()),
        text: "Oi, o checkout está dando erro".into(),
        media: None,
        mentioned_me: false,
        reply_to_me: false,
        at,
        source,
    }
}

#[test]
fn a_contacts_message_wakes_the_agent() {
    let mut exchange = Exchange::new("5511999990000");
    assert_eq!(exchange.observe(&message(Source::Contact, AT), PAUSE_MS), Verdict::Answer);
    assert_eq!(exchange.last_at, Some(AT));
}

#[test]
fn the_agents_own_echo_is_only_kept() {
    let mut exchange = Exchange::new("x");
    assert_eq!(exchange.observe(&message(Source::Agent, AT), PAUSE_MS), Verdict::Keep);
    assert_eq!(exchange.paused_until, None);
}

#[test]
fn a_member_answering_from_the_phone_pauses_the_agent_for_two_hours() {
    let mut exchange = Exchange::new("x");
    assert_eq!(exchange.observe(&message(Source::Member, AT), PAUSE_MS), Verdict::Keep);
    assert_eq!(exchange.observe(&message(Source::Contact, AT + 30 * MINUTE), PAUSE_MS), Verdict::Keep);
    assert_eq!(exchange.observe(&message(Source::Contact, AT + PAUSE_MS), PAUSE_MS), Verdict::Answer);
}

#[test]
fn the_pause_only_moves_forward() {
    let mut exchange = Exchange::new("x");
    exchange.observe(&message(Source::Member, AT), PAUSE_MS);
    exchange.observe(&message(Source::Member, AT - 60 * MINUTE), PAUSE_MS);
    assert_eq!(exchange.paused_until, Some(AT + PAUSE_MS));
    assert_eq!(exchange.last_at, Some(AT));
}

#[test]
fn a_group_message_wakes_the_agent_only_when_meant_for_the_number() {
    let mut exchange = Exchange::new("group@g.us");
    let mut chatter = Message { is_group: true, ..message(Source::Contact, AT) };
    assert_eq!(exchange.observe(&chatter, PAUSE_MS), Verdict::Keep);
    chatter.mentioned_me = true;
    assert_eq!(exchange.observe(&chatter, PAUSE_MS), Verdict::Answer);
    let answer = Message { is_group: true, reply_to_me: true, ..message(Source::Contact, AT) };
    assert_eq!(exchange.observe(&answer, PAUSE_MS), Verdict::Answer);
}

#[test]
fn backlog_is_news_only_since_the_channel_went_offline() {
    assert!(is_news(Delivery::Live, AT, None));
    assert!(is_news(Delivery::Backlog, AT, Some(AT - MINUTE)));
    assert!(!is_news(Delivery::Backlog, AT - 2 * MINUTE, Some(AT - MINUTE)));
    assert!(!is_news(Delivery::Backlog, AT, None), "history read after linking is not news");
}

#[test]
fn a_message_round_trips_through_json() {
    let with_file = Message {
        media: Some(Media {
            kind: MediaKind::Document,
            mime_type: Some("application/pdf".into()),
            filename: None,
            size: None,
            file: None,
        }),
        ..message(Source::Contact, AT)
    };
    let json = serde_json::to_string(&with_file).unwrap_or_default();
    let back: Option<Message> = serde_json::from_str(&json).ok();
    assert_eq!(back, Some(with_file));
}
