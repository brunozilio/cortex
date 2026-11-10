//! Images the model sees: attached to a prompt, read by a tool or returned
//! by an MCP server. Kept base64-encoded with the media type their bytes
//! show; only PNG, JPEG, GIF and WebP are accepted.

use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};

pub const MAX_IMAGE_BYTES: usize = 20 * 1024 * 1024;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Image {
    pub media_type: String,
    pub data: String,
}

impl std::fmt::Debug for Image {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Image").field("media_type", &self.media_type).field("encoded_bytes", &self.data.len()).finish()
    }
}

/// The media type `bytes` start like, when it is an image the model reads.
pub fn media_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        Some("image/webp")
    } else {
        None
    }
}

impl Image {
    /// The image in `bytes`; `None` when they are not one, an error when
    /// it is too large.
    pub fn from_bytes(bytes: &[u8]) -> Result<Option<Self>, String> {
        let Some(media_type) = media_type(bytes) else { return Ok(None) };
        if bytes.len() > MAX_IMAGE_BYTES {
            return Err("image exceeds the 20 MiB upload limit".into());
        }
        Ok(Some(Self { media_type: media_type.into(), data: STANDARD.encode(bytes) }))
    }

    /// An image an MCP server returned, checked against its bytes.
    pub fn from_mcp(media_type: &str, data: &str) -> Result<Self, String> {
        if data.len() > MAX_IMAGE_BYTES * 4 / 3 + 8 {
            return Err("MCP image exceeds the upload limit".into());
        }
        let bytes = STANDARD.decode(data).map_err(|_| "MCP image is not valid base64")?;
        let image = Self::from_bytes(&bytes)?.ok_or("MCP image has an unsupported format")?;
        if image.media_type != media_type {
            return Err("MCP image MIME type does not match its bytes".into());
        }
        Ok(image)
    }

    pub fn data_url(&self) -> String {
        format!("data:{};base64,{}", self.media_type, self.data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n rest";

    #[test]
    fn images_are_known_by_their_bytes_not_their_name() {
        assert_eq!(media_type(PNG), Some("image/png"));
        assert_eq!(media_type(b"RIFF1234WEBPVP8"), Some("image/webp"));
        assert_eq!(media_type(b"plain text"), None);
        assert_eq!(Image::from_bytes(b"plain text"), Ok(None));
        let image = Image::from_bytes(PNG).expect("ok").expect("image");
        assert!(image.data_url().starts_with("data:image/png;base64,"));
    }

    #[test]
    fn an_mcp_image_must_match_its_declared_type() {
        let data = STANDARD.encode(PNG);
        assert!(Image::from_mcp("image/png", &data).is_ok());
        assert!(Image::from_mcp("image/jpeg", &data).is_err());
        assert!(Image::from_mcp("image/png", "!!").is_err());
    }
}
