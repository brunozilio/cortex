//! Meta's webhook payload for WhatsApp (`whatsapp_business_account`) into
//! protocol-free messages. Pure.
//!
//! `messages` are a contact's; `smb_message_echoes` are what a member sent
//! from the WhatsApp Business app on the same number (coexistence), which
//! pauses the agent there. Delivery `statuses` carry no message. Only the
//! changes for `phone_number_id` are taken: one Meta app delivers every
//! number it manages to the same webhook.

use serde_json::Value;
use superfik_core::channel::{Media, MediaKind, Message, Source};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arrived {
    pub message: Message,
    pub media_id: Option<String>,
}

pub fn parse(body: &[u8], phone_number_id: &str) -> Vec<Arrived> {
    let Ok(payload) = serde_json::from_slice::<Value>(body) else { return Vec::new() };
    let changes = payload["entry"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|entry| entry["changes"].as_array().into_iter().flatten());
    let mut arrived = Vec::new();
    for change in changes {
        let value = &change["value"];
        if value["metadata"]["phone_number_id"].as_str() != Some(phone_number_id) {
            continue;
        }
        let name_of = |wa_id: &str| {
            value["contacts"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|c| c["wa_id"].as_str() == Some(wa_id))
                .and_then(|c| c["profile"]["name"].as_str().map(str::to_owned))
        };
        for raw in value["messages"].as_array().into_iter().flatten() {
            let Some(from) = raw["from"].as_str() else { continue };
            if let Some(mut one) = message(raw, from, Source::Contact) {
                one.message.sender = Some(from.to_owned());
                one.message.sender_name = name_of(from);
                arrived.push(one);
            }
        }
        for raw in value["message_echoes"].as_array().into_iter().flatten() {
            let Some(to) = raw["to"].as_str() else { continue };
            arrived.extend(message(raw, to, Source::Member));
        }
    }
    arrived
}

fn message(raw: &Value, exchange: &str, source: Source) -> Option<Arrived> {
    let id = raw["id"].as_str()?.to_owned();
    let at = raw["timestamp"].as_str().and_then(|s| s.parse::<i64>().ok()).map_or(0, |secs| secs * 1_000);
    let kind = raw["type"].as_str().unwrap_or_default();
    let (text, media, media_id) = match kind {
        "text" => (raw["text"]["body"].as_str().unwrap_or_default().to_owned(), None, None),
        "image" | "document" | "audio" | "video" | "sticker" => {
            let part = &raw[kind];
            let mime = part["mime_type"].as_str().map(str::to_owned);
            let media_kind = match kind {
                "image" | "sticker" => MediaKind::Image,
                "audio" => MediaKind::Audio,
                "video" => MediaKind::Video,
                _ => MediaKind::Document,
            };
            let media = Media {
                kind: media_kind,
                mime_type: mime,
                filename: part["filename"].as_str().map(str::to_owned),
                size: None,
                file: None,
            };
            let caption = part["caption"].as_str().unwrap_or_default().to_owned();
            (caption, Some(media), part["id"].as_str().map(str::to_owned))
        }
        "button" => (raw["button"]["text"].as_str().unwrap_or_default().to_owned(), None, None),
        "interactive" => {
            let reply = &raw["interactive"];
            let title = reply["button_reply"]["title"].as_str().or(reply["list_reply"]["title"].as_str());
            (title.unwrap_or_default().to_owned(), None, None)
        }
        "location" => {
            let place = &raw["location"];
            let (lat, lon) =
                (place["latitude"].as_f64().unwrap_or_default(), place["longitude"].as_f64().unwrap_or_default());
            (format!("📍 {lat}, {lon}"), None, None)
        }
        _ => return None,
    };
    if text.is_empty() && media.is_none() {
        return None;
    }
    let message = Message {
        id,
        exchange: exchange.to_owned(),
        is_group: false,
        sender: None,
        sender_name: None,
        text,
        media,
        mentioned_me: false,
        reply_to_me: false,
        at,
        source,
    };
    Some(Arrived { message, media_id })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PHONE: &str = "106540352242922";

    fn fixture(name: &str) -> Vec<u8> {
        let path = format!("{}/tests/fixtures/whatsapp_cloud/{name}", env!("CARGO_MANIFEST_DIR"));
        std::fs::read(path).expect("fixture")
    }

    #[test]
    fn a_contacts_text_is_theirs_named_by_their_profile() {
        let arrived = parse(&fixture("text.json"), PHONE);
        assert_eq!(arrived.len(), 1);
        let m = &arrived[0].message;
        assert_eq!(m.id, "wamid.HBgLMTY1MDM4Nzk0MzkVAgASGBQzQTRBNjU5OUFFRTAzODEwMTQ0RgA=");
        assert_eq!(m.exchange, "5511999990000");
        assert_eq!(m.sender.as_deref(), Some("5511999990000"));
        assert_eq!(m.sender_name.as_deref(), Some("Ana"));
        assert_eq!(m.text, "O checkout não abre");
        assert_eq!(m.at, 1_793_016_000_000);
        assert_eq!(m.source, Source::Contact);
    }

    #[test]
    fn a_file_names_its_media_to_fetch() {
        let arrived = parse(&fixture("document.json"), PHONE);
        let one = &arrived[0];
        assert_eq!(one.media_id.as_deref(), Some("1037543291543636"));
        let media = one.message.media.as_ref().expect("media");
        assert_eq!(media.kind, MediaKind::Document);
        assert_eq!(media.filename.as_deref(), Some("nota.pdf"));
        assert_eq!(one.message.text, "Segue a nota");
    }

    #[test]
    fn statuses_and_other_numbers_carry_nothing() {
        assert!(parse(&fixture("status.json"), PHONE).is_empty());
        assert!(parse(&fixture("text.json"), "another-number").is_empty());
        assert!(parse(b"not json", PHONE).is_empty());
    }

    #[test]
    fn what_a_member_sends_from_the_business_app_is_the_members() {
        let arrived = parse(&fixture("echo.json"), PHONE);
        assert_eq!(arrived.len(), 1);
        assert_eq!(arrived[0].message.source, Source::Member);
        assert_eq!(arrived[0].message.exchange, "5511999990000");
    }
}
