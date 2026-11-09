//! Meta's webhook payload for Instagram (`object: "instagram"`) into
//! protocol-free messages. Pure.
//!
//! - `messaging`: direct messages. A contact's is named by its
//!   Instagram-scoped id (IGSID), the conversation's id. An echo
//!   (`is_echo`) is the account's own message: the agent's when it sent
//!   that `mid`, else a member's from the Instagram app, which pauses the
//!   agent there.
//! - `changes` with `field: "comments"`: a comment on one of the account's
//!   posts, each its own conversation (`comment:<id>`), answered by a reply
//!   under it. The account's own comments are not news.
//!
//! Only entries for `account` (the Instagram account's id) are taken.

use serde_json::Value;
use superfik_core::channel::{Media, MediaKind, Message, Source};

pub const COMMENT: &str = "comment:";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arrived {
    pub message: Message,
    pub file_url: Option<String>,
}

pub fn parse(body: &[u8], account: &str, ours: impl Fn(&str) -> bool) -> Vec<Arrived> {
    let Ok(payload) = serde_json::from_slice::<Value>(body) else { return Vec::new() };
    let entries = payload["entry"].as_array().into_iter().flatten().filter(|e| e["id"].as_str() == Some(account));
    let mut arrived = Vec::new();
    for entry in entries {
        for event in entry["messaging"].as_array().into_iter().flatten() {
            arrived.extend(direct(event, &ours));
        }
        let at = millis(entry["time"].as_i64().unwrap_or_default());
        for change in entry["changes"].as_array().into_iter().flatten() {
            if change["field"].as_str() == Some("comments") {
                arrived.extend(comment(&change["value"], account, at, &ours));
            }
        }
    }
    arrived
}

fn direct(event: &Value, ours: &impl Fn(&str) -> bool) -> Option<Arrived> {
    let raw = &event["message"];
    let id = raw["mid"].as_str()?.to_owned();
    if raw["is_deleted"].as_bool() == Some(true) {
        return None;
    }
    let echo = raw["is_echo"].as_bool() == Some(true);
    let (exchange, source) = if echo {
        let source = if ours(&id) { Source::Agent } else { Source::Member };
        (event["recipient"]["id"].as_str()?, source)
    } else {
        (event["sender"]["id"].as_str()?, Source::Contact)
    };
    let attachment = raw["attachments"].as_array().and_then(|all| all.first());
    let media = attachment.map(|a| Media {
        kind: match a["type"].as_str().unwrap_or_default() {
            "image" | "story_mention" => MediaKind::Image,
            "video" | "reel" | "ig_reel" => MediaKind::Video,
            "audio" => MediaKind::Audio,
            _ => MediaKind::Document,
        },
        mime_type: None,
        filename: None,
        size: None,
        file: None,
    });
    let file_url = attachment.and_then(|a| a["payload"]["url"].as_str()).map(str::to_owned);
    let text = raw["text"].as_str().unwrap_or_default().to_owned();
    if text.is_empty() && media.is_none() {
        return None;
    }
    let message = Message {
        id,
        exchange: exchange.to_owned(),
        is_group: false,
        sender: (source == Source::Contact).then(|| exchange.to_owned()),
        sender_name: None,
        text,
        media,
        mentioned_me: false,
        reply_to_me: false,
        at: event["timestamp"].as_i64().unwrap_or_default(),
        source,
    };
    Some(Arrived { message, file_url })
}

fn millis(time: i64) -> i64 {
    if time < 100_000_000_000 { time * 1_000 } else { time }
}

fn comment(value: &Value, account: &str, at: i64, ours: &impl Fn(&str) -> bool) -> Option<Arrived> {
    let id = value["id"].as_str()?;
    let text = value["text"].as_str().unwrap_or_default();
    let from = value["from"]["id"].as_str()?;
    let source = match from == account {
        true if ours(id) => Source::Agent,
        true => Source::Member,
        false => Source::Contact,
    };
    let thread = value["parent_id"].as_str().unwrap_or(id);
    let message = Message {
        id: id.to_owned(),
        exchange: format!("{COMMENT}{thread}"),
        is_group: false,
        sender: Some(from.to_owned()),
        sender_name: value["from"]["username"].as_str().map(str::to_owned),
        text: text.to_owned(),
        media: None,
        mentioned_me: false,
        reply_to_me: false,
        at,
        source,
    };
    (!text.is_empty()).then_some(Arrived { message, file_url: None })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACCOUNT: &str = "17841405822304914";

    fn fixture(name: &str) -> Vec<u8> {
        std::fs::read(format!("{}/tests/fixtures/instagram/{name}", env!("CARGO_MANIFEST_DIR"))).expect("fixture")
    }

    #[test]
    fn a_direct_message_is_the_contacts_in_their_conversation() {
        let arrived = parse(&fixture("message.json"), ACCOUNT, |_| false);
        assert_eq!(arrived.len(), 1);
        let m = &arrived[0].message;
        assert_eq!(m.exchange, "6857162164316212");
        assert_eq!(m.source, Source::Contact);
        assert_eq!(m.text, "Vocês entregam em Recife?");
        assert_eq!(m.at, 1_793_016_000_000);
    }

    #[test]
    fn an_echo_is_the_agents_when_it_sent_it_else_a_members() {
        let mine = parse(&fixture("echo.json"), ACCOUNT, |id| id == "aWdfZAG1faXRlbTpJR01lc3NhZA");
        assert_eq!(mine[0].message.source, Source::Agent);
        assert_eq!(mine[0].message.exchange, "6857162164316212");
        let theirs = parse(&fixture("echo.json"), ACCOUNT, |_| false);
        assert_eq!(theirs[0].message.source, Source::Member);
    }

    #[test]
    fn an_image_names_its_address() {
        let arrived = parse(&fixture("image.json"), ACCOUNT, |_| false);
        assert_eq!(arrived[0].file_url.as_deref(), Some("https://lookaside.fbsbx.com/ig_messaging_cdn/1"));
        assert_eq!(arrived[0].message.media.as_ref().map(|m| m.kind), Some(MediaKind::Image));
    }

    #[test]
    fn a_comment_is_its_own_conversation_and_replies_join_it() {
        let arrived = parse(&fixture("comment.json"), ACCOUNT, |_| false);
        assert_eq!(arrived.len(), 2);
        assert_eq!(arrived[0].message.exchange, "comment:17865799348089039");
        assert_eq!(arrived[0].message.sender_name.as_deref(), Some("ana.lima"));
        assert_eq!(arrived[0].message.source, Source::Contact);
        assert_eq!(arrived[0].message.at, 1_793_016_300_000, "the entry's time, in seconds");
        assert_eq!(arrived[1].message.exchange, "comment:17865799348089039", "a reply under it");
        assert_eq!(arrived[1].message.source, Source::Member, "the account's own reply, not the agent's");
    }

    #[test]
    fn other_accounts_and_noise_carry_nothing() {
        assert!(parse(&fixture("message.json"), "another", |_| false).is_empty());
        assert!(parse(b"[]", ACCOUNT, |_| false).is_empty());
    }
}
