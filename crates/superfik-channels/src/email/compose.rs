//! The e-mail a reply goes out as: in the conversation's thread (subject,
//! In-Reply-To, References), plain text, and the file as an attachment.

use lettre::message::header::ContentType;
use lettre::message::{Attachment, MultiPart, SinglePart};

use super::parse::{Threading, reply_subject};
use crate::{ChannelError, OutgoingFile};

pub fn compose(
    from: &str,
    to: &str,
    threading: &Threading,
    id: &str,
    text: &str,
    file: Option<&OutgoingFile<'_>>,
) -> Result<lettre::Message, ChannelError> {
    let address = |address: &str| address.parse().map_err(|_| ChannelError::Address(address.to_owned()));
    let mut builder = lettre::Message::builder()
        .from(address(from)?)
        .to(address(to)?)
        .subject(reply_subject(&threading.subject))
        .message_id(Some(id.to_owned()));
    if let Some(parent) = &threading.message_id {
        builder = builder.in_reply_to(format!("<{parent}>"));
    }
    if !threading.references.is_empty() {
        let references: Vec<String> = threading.references.iter().map(|r| format!("<{r}>")).collect();
        builder = builder.references(references.join(" "));
    }
    let failed = |error: lettre::error::Error| ChannelError::Failed(error.to_string());
    let Some(file) = file else {
        return builder.header(ContentType::TEXT_PLAIN).body(text.to_owned()).map_err(failed);
    };
    let mime = file.media.mime_type.as_deref().unwrap_or("application/octet-stream");
    let content_type = ContentType::parse(mime).unwrap_or(
        ContentType::parse("application/octet-stream").map_err(|error| ChannelError::Failed(error.to_string()))?,
    );
    let name = file.media.filename.clone().unwrap_or_else(|| "file".to_owned());
    let attachment = Attachment::new(name).body(file.bytes.to_vec(), content_type);
    let body = MultiPart::mixed().singlepart(SinglePart::plain(text.to_owned())).singlepart(attachment);
    builder.multipart(body).map_err(failed)
}

#[cfg(test)]
mod tests {
    use superfik_core::channel::{Media, MediaKind};

    use super::*;

    #[test]
    fn a_reply_with_a_file_stays_in_the_thread_and_carries_it() {
        let threading = Threading {
            subject: "Nota".into(),
            message_id: Some("abc@example.com".into()),
            references: vec!["abc@example.com".into()],
        };
        let media = Media {
            kind: MediaKind::Document,
            mime_type: Some("application/pdf".into()),
            filename: Some("nota.pdf".into()),
            size: Some(8),
            file: None,
        };
        let file = OutgoingFile { media: &media, bytes: b"%PDF-1.4", caption: "Aqui está." };
        let email =
            compose("support@product.com", "ana@example.com", &threading, "<x@superfik>", "Aqui está.", Some(&file))
                .unwrap_or_else(|error| panic!("{error}"));
        let raw = String::from_utf8_lossy(&email.formatted()).into_owned();
        assert!(raw.contains("Subject: Re: Nota"));
        assert!(raw.contains("In-Reply-To: <abc@example.com>"));
        assert!(raw.contains("multipart/mixed"));
        assert!(raw.contains("filename=\"nota.pdf\""));
        assert!(raw.contains("attachment") && raw.contains("nota.pdf"));
        let (parsed, _, kept) = super::super::parse::parse(&email.formatted(), "ana@example.com")
            .unwrap_or_else(|| panic!("not read back"));
        assert_eq!(kept.as_deref(), Some(&b"%PDF-1.4"[..]));
        assert!(parsed.text.contains("Aqui está."));
    }

    #[test]
    fn a_plain_reply_has_no_parts() {
        let email = compose("a@x.com", "b@y.com", &Threading::default(), "<i@superfik>", "Oi", None)
            .unwrap_or_else(|error| panic!("{error}"));
        let raw = String::from_utf8_lossy(&email.formatted()).into_owned();
        assert!(raw.contains("Content-Type: text/plain"));
        assert!(!raw.contains("multipart"));
    }
}
