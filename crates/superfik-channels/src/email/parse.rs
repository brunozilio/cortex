//! A raw e-mail in Superfik's shape: one conversation per correspondent
//! (their address), the text without the quoted history below it, and what
//! a reply needs to stay in the same thread (the subject and message ids).
//! Pure, so it is tested on raw messages.

use mail_parser::MessageParser;
use superfik_core::channel::{MAX_MEDIA_BYTES, Media, MediaKind, Message, Source};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Threading {
    pub subject: String,
    pub message_id: Option<String>,
    pub references: Vec<String>,
}

const KEPT_CHARS: usize = 8_000;

pub fn parse(raw: &[u8], me: &str) -> Option<(Message, Threading, Option<Vec<u8>>)> {
    let parsed = MessageParser::default().parse(raw)?;
    let from = parsed.from()?.first()?;
    let address = from.address()?.trim().to_lowercase();
    if address.is_empty() || address == me.trim().to_lowercase() {
        return None;
    }
    let subject = parsed.subject().unwrap_or_default().trim().to_owned();
    let body = parsed.body_text(0).map(|text| reply_only(&text)).unwrap_or_default();
    let text = match (subject.is_empty(), body.is_empty()) {
        (false, false) => format!("{subject}\n\n{body}"),
        (false, true) => subject.clone(),
        _ => body,
    };
    let text: String = text.chars().take(KEPT_CHARS).collect();
    let message_id = parsed.message_id().map(|id| id.trim().to_owned());
    let at = parsed.date().map_or(0, |date| date.to_timestamp() * 1_000);
    let mut references: Vec<String> =
        parsed.references().map(|list| list.iter().map(|id| id.trim().to_owned()).collect()).unwrap_or_default();
    references.extend(message_id.clone());
    let attachment = parsed.attachments().next();
    let media = attachment.as_ref().map(|part| {
        let mime = part.content_type().map(|t| match t.subtype() {
            Some(sub) => format!("{}/{sub}", t.ctype()),
            None => t.ctype().to_owned(),
        });
        Media {
            kind: mime.as_deref().map_or(MediaKind::Document, MediaKind::of_mime),
            mime_type: mime,
            filename: part.attachment_name().map(str::to_owned),
            size: Some(part.decoded_len() as u64),
            file: None,
        }
    });
    let bytes =
        attachment.filter(|part| part.decoded_len() as u64 <= MAX_MEDIA_BYTES).map(|part| part.decoded().into_owned());
    let message = Message {
        id: message_id.clone().unwrap_or_else(|| format!("{address}-{at}")),
        exchange: address.clone(),
        is_group: false,
        sender: Some(address),
        sender_name: from.name().map(|name| name.trim().to_owned()).filter(|name| !name.is_empty()),
        text,
        media,
        mentioned_me: false,
        reply_to_me: false,
        at,
        source: Source::Contact,
    };
    Some((message, Threading { subject, message_id, references }, bytes))
}

pub fn reply_only(body: &str) -> String {
    let mut kept = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim();
        let attribution = (trimmed.starts_with("On ") && trimmed.ends_with("wrote:"))
            || (trimmed.starts_with("Em ") && trimmed.ends_with("escreveu:"))
            || trimmed.starts_with("-----Original Message-----");
        if attribution || trimmed.starts_with('>') {
            break;
        }
        kept.push(line);
    }
    kept.join("\n").trim().to_owned()
}

pub fn reply_subject(subject: &str) -> String {
    let subject = subject.trim();
    let lower = subject.to_lowercase();
    if lower.starts_with("re:") || lower.starts_with("res:") {
        subject.to_owned()
    } else if subject.is_empty() {
        "Re:".to_owned()
    } else {
        format!("Re: {subject}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RAW: &[u8] = b"From: Ana Souza <Ana@Example.com>\r\n\
To: support@product.com\r\n\
Subject: Checkout fails with a coupon\r\n\
Message-ID: <abc@example.com>\r\n\
References: <first@product.com>\r\n\
Date: Mon, 26 Oct 2026 12:00:00 +0000\r\n\
Content-Type: text/plain; charset=utf-8\r\n\
\r\n\
Hi, the checkout breaks when I use a coupon.\r\n\
\r\n\
On Sun, Oct 25, 2026 at 9:00 AM Support <support@product.com> wrote:\r\n\
> Thanks for writing to us.\r\n";

    #[test]
    fn an_email_is_a_message_from_its_sender_without_the_quoted_history() {
        let (message, threading, file) = parse(RAW, "support@product.com").unwrap_or_else(|| panic!("not parsed"));
        assert_eq!((message.media, file), (None, None));
        assert_eq!(message.exchange, "ana@example.com");
        assert_eq!(message.sender_name.as_deref(), Some("Ana Souza"));
        assert_eq!(message.text, "Checkout fails with a coupon\n\nHi, the checkout breaks when I use a coupon.");
        assert_eq!(message.at, 1_793_016_000_000);
        assert_eq!(message.source, Source::Contact);
        assert_eq!(threading.subject, "Checkout fails with a coupon");
        assert_eq!(threading.message_id.as_deref(), Some("abc@example.com"));
        assert_eq!(threading.references, ["first@product.com", "abc@example.com"]);
    }

    const WITH_FILE: &[u8] = b"From: Ana <ana@example.com>\r\n\
Subject: Nota\r\n\
MIME-Version: 1.0\r\n\
Content-Type: multipart/mixed; boundary=\"b1\"\r\n\
\r\n\
--b1\r\n\
Content-Type: text/plain; charset=utf-8\r\n\
\r\n\
Segue a nota fiscal.\r\n\
--b1\r\n\
Content-Type: application/pdf; name=\"nota.pdf\"\r\n\
Content-Disposition: attachment; filename=\"nota.pdf\"\r\n\
Content-Transfer-Encoding: base64\r\n\
\r\n\
JVBERi0xLjQ=\r\n\
--b1--\r\n";

    #[test]
    fn an_attached_file_comes_with_the_message() {
        let (message, _, file) = parse(WITH_FILE, "support@product.com").unwrap_or_else(|| panic!("not parsed"));
        let media = message.media.unwrap_or_else(|| panic!("no media"));
        assert_eq!(media.kind, MediaKind::Document);
        assert_eq!(media.mime_type.as_deref(), Some("application/pdf"));
        assert_eq!(media.filename.as_deref(), Some("nota.pdf"));
        assert_eq!(file.as_deref(), Some(&b"%PDF-1.4"[..]));
        assert_eq!(media.size, Some(8));
        assert!(message.text.contains("Segue a nota fiscal."));
    }

    #[test]
    fn the_mailboxs_own_messages_are_not_a_contacts() {
        let own = b"From: Support <SUPPORT@product.com>\r\nSubject: x\r\n\r\nhello\r\n";
        assert!(parse(own, "support@product.com").is_none());
        assert!(parse(b"not an email at all", "me@x.com").is_none());
    }

    #[test]
    fn replies_keep_one_re_and_lose_the_quote() {
        assert_eq!(reply_subject("Checkout fails"), "Re: Checkout fails");
        assert_eq!(reply_subject("RE: Checkout fails"), "RE: Checkout fails");
        assert_eq!(reply_only("Obrigado!\n\nEm seg., 26 de out. Ana escreveu:\n> oi"), "Obrigado!");
    }
}
