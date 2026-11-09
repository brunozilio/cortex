//! Files on WhatsApp: what a message carries, fetching it from the CDN
//! (decrypted by whatsapp-rust), and the message that sends one.

use superfik_core::channel::{MAX_MEDIA_BYTES, Media, MediaKind};
use whatsapp_rust::Client;
use whatsapp_rust::buffa::MessageField;
use whatsapp_rust::download::{Downloadable, MediaType};
use whatsapp_rust::prelude::MessageExt;
use whatsapp_rust::upload::UploadResponse;
use whatsapp_rust::waproto::whatsapp as wa;

use crate::OutgoingFile;

fn described(kind: MediaKind, mime: Option<&str>, filename: Option<&str>, size: Option<u64>) -> Media {
    Media {
        kind,
        mime_type: mime.map(str::to_owned),
        filename: filename.map(str::to_owned).filter(|name| !name.trim().is_empty()),
        size,
        file: None,
    }
}

pub fn media_in(message: &wa::Message) -> Option<Media> {
    let base = message.get_base_message();
    if let Some(m) = base.image_message.as_option() {
        return Some(described(MediaKind::Image, m.mimetype.as_deref(), None, m.file_length));
    }
    if let Some(m) = base.document_message.as_option() {
        let name = m.file_name.as_deref();
        return Some(described(MediaKind::Document, m.mimetype.as_deref(), name, m.file_length));
    }
    if let Some(m) = base.audio_message.as_option() {
        return Some(described(MediaKind::Audio, m.mimetype.as_deref(), None, m.file_length));
    }
    if let Some(m) = base.video_message.as_option() {
        return Some(described(MediaKind::Video, m.mimetype.as_deref(), None, m.file_length));
    }
    None
}

pub fn caption_in(message: &wa::Message) -> Option<String> {
    let base = message.get_base_message();
    base.image_message
        .as_option()
        .and_then(|m| m.caption.clone())
        .or_else(|| base.document_message.as_option().and_then(|m| m.caption.clone()))
        .or_else(|| base.video_message.as_option().and_then(|m| m.caption.clone()))
}

fn downloadable(message: &wa::Message) -> Option<&dyn Downloadable> {
    let base = message.get_base_message();
    if let Some(m) = base.image_message.as_option() {
        return Some(m);
    }
    if let Some(m) = base.document_message.as_option() {
        return Some(m);
    }
    if let Some(m) = base.audio_message.as_option() {
        return Some(m);
    }
    base.video_message.as_option().map(|m| m as &dyn Downloadable)
}

pub async fn fetch(client: &Client, message: &wa::Message) -> Option<Vec<u8>> {
    let downloadable = downloadable(message)?;
    if downloadable.file_length().is_some_and(|length| length > MAX_MEDIA_BYTES) {
        return None;
    }
    match client.download(downloadable).await {
        Ok(bytes) if bytes.len() as u64 <= MAX_MEDIA_BYTES => Some(bytes),
        Ok(_) => None,
        Err(error) => {
            tracing::warn!(%error, "could not fetch a WhatsApp file");
            None
        }
    }
}

pub fn media_type(kind: MediaKind) -> MediaType {
    match kind {
        MediaKind::Image => MediaType::Image,
        MediaKind::Audio => MediaType::Audio,
        MediaKind::Video => MediaType::Video,
        MediaKind::Document => MediaType::Document,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Uploaded {
    pub url: String,
    pub direct_path: String,
    pub media_key: [u8; 32],
    pub file_sha256: [u8; 32],
    pub file_enc_sha256: [u8; 32],
    pub file_length: u64,
    pub media_key_timestamp: i64,
    pub streaming_sidecar: Option<Vec<u8>>,
}

impl From<UploadResponse> for Uploaded {
    fn from(r: UploadResponse) -> Self {
        Self {
            url: r.url,
            direct_path: r.direct_path,
            media_key: r.media_key,
            file_sha256: r.file_sha256,
            file_enc_sha256: r.file_enc_sha256,
            file_length: r.file_length,
            media_key_timestamp: r.media_key_timestamp,
            streaming_sidecar: r.streaming_sidecar,
        }
    }
}

pub fn message_for(file: &OutgoingFile<'_>, upload: &Uploaded) -> wa::Message {
    let mime = file.media.mime_type.clone();
    let caption = Some(file.caption.to_owned()).filter(|caption| !caption.trim().is_empty());
    let (url, direct_path) = (Some(upload.url.clone()), Some(upload.direct_path.clone()));
    let (media_key, file_sha256, file_enc_sha256) =
        (Some(upload.media_key.to_vec()), Some(upload.file_sha256.to_vec()), Some(upload.file_enc_sha256.to_vec()));
    let (file_length, media_key_timestamp) = (Some(upload.file_length), Some(upload.media_key_timestamp));
    match file.media.kind {
        MediaKind::Image => wa::Message {
            image_message: MessageField::some(wa::message::ImageMessage {
                url,
                mimetype: mime,
                caption,
                file_sha256,
                file_length,
                media_key,
                file_enc_sha256,
                direct_path,
                media_key_timestamp,
                ..Default::default()
            }),
            ..Default::default()
        },
        MediaKind::Video => wa::Message {
            video_message: MessageField::some(wa::message::VideoMessage {
                url,
                mimetype: mime,
                caption,
                file_sha256,
                file_length,
                media_key,
                file_enc_sha256,
                direct_path,
                media_key_timestamp,
                streaming_sidecar: upload.streaming_sidecar.clone(),
                ..Default::default()
            }),
            ..Default::default()
        },
        MediaKind::Audio => wa::Message {
            audio_message: MessageField::some(wa::message::AudioMessage {
                url,
                mimetype: mime,
                file_sha256,
                file_length,
                media_key,
                file_enc_sha256,
                direct_path,
                media_key_timestamp,
                streaming_sidecar: upload.streaming_sidecar.clone(),
                ..Default::default()
            }),
            ..Default::default()
        },
        MediaKind::Document => wa::Message {
            document_message: MessageField::some(wa::message::DocumentMessage {
                url,
                mimetype: mime,
                title: file.media.filename.clone(),
                file_name: file.media.filename.clone(),
                caption,
                file_sha256,
                file_length,
                media_key,
                file_enc_sha256,
                direct_path,
                media_key_timestamp,
                ..Default::default()
            }),
            ..Default::default()
        },
    }
}

#[cfg(test)]
mod tests {
    use whatsapp_rust::prelude::MessageBuilderExt;

    use super::*;

    #[test]
    fn a_document_is_described_by_its_name_and_type() {
        let message = wa::Message {
            document_message: MessageField::some(wa::message::DocumentMessage {
                mimetype: Some("application/pdf".into()),
                file_name: Some("Nota.pdf".into()),
                file_length: Some(2_048),
                caption: Some("segue a nota".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let media = media_in(&message).unwrap_or_else(|| panic!("no media"));
        assert_eq!(media.kind, MediaKind::Document);
        assert_eq!(media.filename.as_deref(), Some("Nota.pdf"));
        assert_eq!(media.size, Some(2_048));
        assert_eq!(caption_in(&message).as_deref(), Some("segue a nota"));
        assert_eq!(media_in(&wa::Message::text("oi")), None);
    }

    #[test]
    fn an_image_goes_out_with_its_upload_and_caption() {
        let media = described(MediaKind::Image, Some("image/png"), Some("chart.png"), Some(3));
        let file = OutgoingFile { media: &media, bytes: b"png", caption: "Here it is" };
        let upload = Uploaded {
            url: "https://mmg.whatsapp.net/x".into(),
            direct_path: "/v/x".into(),
            media_key: [1; 32],
            file_enc_sha256: [2; 32],
            file_sha256: [3; 32],
            file_length: 3,
            media_key_timestamp: 1_700_000_000,
            streaming_sidecar: None,
        };
        let message = message_for(&file, &upload);
        let image = message.image_message.as_option().unwrap_or_else(|| panic!("no image"));
        assert_eq!(image.caption.as_deref(), Some("Here it is"));
        assert_eq!(image.direct_path.as_deref(), Some("/v/x"));
        assert_eq!(image.media_key.as_deref(), Some(&[1u8; 32][..]));
        assert_eq!(media_in(&message).map(|m| m.kind), Some(MediaKind::Image));
    }
}
