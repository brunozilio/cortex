//! Files on channels: what a received one is called on the server, what a
//! file to send is, and the cap on both. Pure.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{Media, MediaKind};

pub const MAX_MEDIA_BYTES: u64 = 16 * 1024 * 1024;

impl MediaKind {
    pub fn of_mime(mime: &str) -> Self {
        match mime.split('/').next().unwrap_or_default().trim().to_ascii_lowercase().as_str() {
            "image" => Self::Image,
            "audio" => Self::Audio,
            "video" => Self::Video,
            _ => Self::Document,
        }
    }
}

pub fn mime_of(name: &str) -> &'static str {
    let extension = Path::new(name).extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase();
    match extension.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "pdf" => "application/pdf",
        "txt" | "md" => "text/plain",
        "csv" => "text/csv",
        "json" => "application/json",
        "zip" => "application/zip",
        "mp3" => "audio/mpeg",
        "ogg" | "opus" => "audio/ogg",
        "m4a" => "audio/mp4",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        _ => "application/octet-stream",
    }
}

fn extension_of(mime: &str) -> &'static str {
    match mime {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "application/pdf" => "pdf",
        "text/plain" => "txt",
        "audio/mpeg" => "mp3",
        "audio/ogg" | "audio/ogg; codecs=opus" => "ogg",
        "video/mp4" => "mp4",
        _ => "bin",
    }
}

pub fn kept_name(message: &str, media: &Media) -> String {
    let clean = |text: &str| -> String {
        text.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '-' | '_') { c } else { '_' }).collect()
    };
    let id = clean(message);
    let id: String = id.chars().take(64).collect();
    match media.filename.as_deref().map(|name| clean_file(&clean, name)).filter(|name| !name.is_empty()) {
        Some(name) => format!("{id}-{name}"),
        None => format!("{id}.{}", extension_of(media.mime_type.as_deref().unwrap_or_default())),
    }
}

fn clean_file(clean: &impl Fn(&str) -> String, name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or_default();
    let path = Path::new(base);
    let stem = clean(path.file_stem().and_then(|s| s.to_str()).unwrap_or_default());
    let stem: String = stem.chars().take(80).collect();
    match path.extension().and_then(|e| e.to_str()).map(clean) {
        Some(extension) if !extension.is_empty() && !stem.is_empty() => format!("{stem}.{extension}"),
        _ => stem,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attachment {
    pub path: PathBuf,
    pub media: Media,
}

impl Attachment {
    pub fn of(path: PathBuf, size: u64) -> Self {
        let filename = path.file_name().and_then(|n| n.to_str()).map(str::to_owned);
        let mime = mime_of(filename.as_deref().unwrap_or_default());
        let media = Media {
            kind: MediaKind::of_mime(mime),
            mime_type: Some(mime.to_owned()),
            filename,
            size: Some(size),
            file: None,
        };
        Self { path, media }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn media(filename: Option<&str>, mime: Option<&str>) -> Media {
        Media {
            kind: MediaKind::Document,
            mime_type: mime.map(Into::into),
            filename: filename.map(Into::into),
            size: None,
            file: None,
        }
    }

    #[test]
    fn kinds_and_types_follow_the_name() {
        assert_eq!(MediaKind::of_mime("image/png"), MediaKind::Image);
        assert_eq!(MediaKind::of_mime("audio/ogg; codecs=opus"), MediaKind::Audio);
        assert_eq!(MediaKind::of_mime("application/pdf"), MediaKind::Document);
        assert_eq!(mime_of("Invoice.PDF"), "application/pdf");
        assert_eq!(mime_of("photo.jpeg"), "image/jpeg");
        assert_eq!(mime_of("no-extension"), "application/octet-stream");
    }

    #[test]
    fn a_kept_name_cannot_leave_its_folder() {
        let name = kept_name("../../etc/passwd", &media(Some("../../.ssh/id_rsa.pub"), None));
        assert!(!name.contains('/') && !name.contains(".."), "{name}");
        assert_eq!(name, "______etc_passwd-id_rsa.pub");
        assert_eq!(kept_name("3EB0A1", &media(None, Some("image/jpeg"))), "3EB0A1.jpg");
        assert_eq!(kept_name("3EB0A1", &media(Some("Nota fiscal.pdf"), None)), "3EB0A1-Nota_fiscal.pdf");
    }

    #[test]
    fn an_attachment_is_described_by_its_file() {
        let attachment = Attachment::of(PathBuf::from("/repo/out/chart.png"), 2_048);
        assert_eq!(attachment.media.kind, MediaKind::Image);
        assert_eq!(attachment.media.mime_type.as_deref(), Some("image/png"));
        assert_eq!(attachment.media.filename.as_deref(), Some("chart.png"));
        assert_eq!(attachment.media.size, Some(2_048));
    }
}
