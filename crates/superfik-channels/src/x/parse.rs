//! X's answers into protocol-free messages. Pure.
//!
//! Conversations: `dm:<user id>` for direct messages with that person,
//! `post:<conversation id>` for a public thread the account was mentioned
//! in (answered by a reply to its latest post). `me` is the account's user
//! id; `ours` says whether a message id is one the agent sent.

use serde_json::Value;
use superfik_core::channel::{Message, Source};

pub const DM: &str = "dm:";
pub const POST: &str = "post:";

fn message(id: &str, exchange: String, text: &str, at: i64, source: Source) -> Message {
    Message {
        id: id.to_owned(),
        exchange,
        is_group: false,
        sender: None,
        sender_name: None,
        text: text.to_owned(),
        media: None,
        mentioned_me: false,
        reply_to_me: false,
        at,
        source,
    }
}

fn name_in(users: &Value, id: &str) -> Option<String> {
    users.as_array()?.iter().find(|u| u["id"].as_str() == Some(id)).and_then(|u| u["name"].as_str()).map(str::to_owned)
}

pub fn mentions(body: &Value, me: &str) -> Vec<Message> {
    let users = &body["includes"]["users"];
    body["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|post| {
            let (id, author) = (post["id"].as_str()?, post["author_id"].as_str()?);
            if author == me {
                return None;
            }
            let thread = post["conversation_id"].as_str().unwrap_or(id);
            let at = post["created_at"].as_str().and_then(rfc3339_ms).unwrap_or_default();
            let mut m = message(id, format!("{POST}{thread}"), post["text"].as_str()?, at, Source::Contact);
            m.is_group = true;
            m.mentioned_me = true;
            m.sender = Some(author.to_owned());
            m.sender_name = name_in(users, author);
            Some(m)
        })
        .collect()
}

pub fn dm_events(body: &Value, me: &str, ours: impl Fn(&str) -> bool) -> Vec<Message> {
    let users = &body["includes"]["users"];
    body["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|event| event["event_type"].as_str() == Some("MessageCreate"))
        .filter_map(|event| {
            let (id, sender) = (event["id"].as_str()?, event["sender_id"].as_str()?);
            let at = event["created_at"].as_str().and_then(rfc3339_ms).unwrap_or_default();
            let text = event["text"].as_str().unwrap_or_default();
            if sender == me {
                let pair = event["dm_conversation_id"].as_str()?;
                let other = pair.split('-').find(|part| *part != me)?;
                let source = if ours(id) { Source::Agent } else { Source::Member };
                return Some(message(id, format!("{DM}{other}"), text, at, source));
            }
            let mut m = message(id, format!("{DM}{sender}"), text, at, Source::Contact);
            m.sender = Some(sender.to_owned());
            m.sender_name = name_in(users, sender);
            Some(m)
        })
        .collect()
}

pub fn activity(body: &[u8], me: &str, ours: impl Fn(&str) -> bool) -> Vec<Message> {
    let Ok(event) = serde_json::from_slice::<Value>(body) else { return Vec::new() };
    if event["for_user_id"].as_str() != Some(me) {
        return Vec::new();
    }
    let mut found = Vec::new();
    for post in event["tweet_create_events"].as_array().into_iter().flatten() {
        let (Some(id), Some(author)) = (post["id_str"].as_str(), post["user"]["id_str"].as_str()) else { continue };
        if author == me {
            continue;
        }
        let thread = post["in_reply_to_status_id_str"].as_str().unwrap_or(id);
        let at = post["timestamp_ms"].as_str().and_then(|ms| ms.parse().ok()).unwrap_or_default();
        let text = post["extended_tweet"]["full_text"].as_str().or(post["text"].as_str()).unwrap_or_default();
        let mut m = message(id, format!("{POST}{thread}"), text, at, Source::Contact);
        m.is_group = true;
        m.mentioned_me = true;
        m.sender = Some(author.to_owned());
        m.sender_name = post["user"]["name"].as_str().map(str::to_owned);
        found.push(m);
    }
    let users = &event["users"];
    for dm in event["direct_message_events"].as_array().into_iter().flatten() {
        let create = &dm["message_create"];
        let (Some(id), Some(sender), Some(target)) =
            (dm["id"].as_str(), create["sender_id"].as_str(), create["target"]["recipient_id"].as_str())
        else {
            continue;
        };
        let at = dm["created_timestamp"].as_str().and_then(|ms| ms.parse().ok()).unwrap_or_default();
        let text = create["message_data"]["text"].as_str().unwrap_or_default();
        if sender == me {
            let source = if ours(id) { Source::Agent } else { Source::Member };
            found.push(message(id, format!("{DM}{target}"), text, at, source));
        } else {
            let mut m = message(id, format!("{DM}{sender}"), text, at, Source::Contact);
            m.sender = Some(sender.to_owned());
            m.sender_name = users[sender]["name"].as_str().map(str::to_owned);
            found.push(m);
        }
    }
    found
}

pub fn rfc3339_ms(text: &str) -> Option<i64> {
    let (date, time) = text.trim_end_matches('Z').split_once('T')?;
    let mut ymd = date.split('-').map(|p| p.parse::<i64>().ok());
    let (y, m, d) = (ymd.next()??, ymd.next()??, ymd.next()??);
    let (clock, fraction) = time.split_once('.').unwrap_or((time, "0"));
    let mut hms = clock.split(':').map(|p| p.parse::<i64>().ok());
    let (hh, mm, ss) = (hms.next()??, hms.next()??, hms.next()??);
    let millis = format!("{fraction:0<3}").get(..3)?.parse::<i64>().ok()?;
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(((days * 24 + hh) * 60 + mm) * 60_000 + ss * 1_000 + millis)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ME: &str = "1580000000000000001";

    fn fixture(name: &str) -> Value {
        let path = format!("{}/tests/fixtures/x/{name}", env!("CARGO_MANIFEST_DIR"));
        serde_json::from_slice(&std::fs::read(path).expect("fixture")).expect("json")
    }

    #[test]
    fn times_are_read_in_utc_milliseconds() {
        assert_eq!(rfc3339_ms("2026-10-26T12:00:00.000Z"), Some(1_793_016_000_000));
        assert_eq!(rfc3339_ms("1970-01-01T00:00:01Z"), Some(1_000));
        assert_eq!(rfc3339_ms("yesterday"), None);
    }

    #[test]
    fn a_mention_is_meant_for_the_account_in_its_thread() {
        let found = mentions(&fixture("mentions.json"), ME);
        assert_eq!(found.len(), 1, "the account's own post is left out");
        let m = &found[0];
        assert_eq!(m.exchange, "post:1850000000000000000");
        assert_eq!(m.sender_name.as_deref(), Some("Ana Lima"));
        assert!(m.is_group && m.mentioned_me);
        assert_eq!(m.at, 1_793_016_000_000);
    }

    #[test]
    fn dms_are_named_by_the_other_person_and_echoes_by_who_sent_them() {
        let found = dm_events(&fixture("dm_events.json"), ME, |id| id == "1850000000000000102");
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].exchange, "dm:2244994945");
        assert_eq!(found[0].source, Source::Agent);
        assert_eq!(found[1].source, Source::Contact);
        assert_eq!(found[1].sender_name.as_deref(), Some("Ana Lima"));
    }

    #[test]
    fn account_activity_brings_mentions_and_dms_for_its_user_only() {
        let body = std::fs::read(format!("{}/tests/fixtures/x/activity.json", env!("CARGO_MANIFEST_DIR"))).expect("f");
        let found = activity(&body, ME, |_| false);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].exchange, "post:1850000000000000000");
        assert_eq!(found[1].exchange, "dm:2244994945");
        assert_eq!(found[1].sender_name.as_deref(), Some("Ana Lima"));
        assert!(activity(&body, "someone-else", |_| false).is_empty());
    }
}
