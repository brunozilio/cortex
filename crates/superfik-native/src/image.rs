//! Image payloads travel separately from text through tools and transcripts.
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use std::io::{Read, Seek};
use std::path::Path;

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
impl Image {
    pub fn from_bytes(bytes: &[u8]) -> Result<Option<Self>, String> {
        let media_type = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            "image/png"
        } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
            "image/jpeg"
        } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
            "image/gif"
        } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
            "image/webp"
        } else {
            return Ok(None);
        };
        if bytes.len() > MAX_IMAGE_BYTES {
            return Err("image exceeds the 20 MiB upload limit".into());
        }
        Ok(Some(Self { media_type: media_type.into(), data: STANDARD.encode(bytes) }))
    }
    pub fn load(path: &Path) -> Result<Option<Self>, String> {
        let metadata = std::fs::metadata(path).map_err(|e| e.to_string())?;
        if !metadata.is_file() {
            return Ok(None);
        }
        let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        let mut header = Vec::new();
        Read::by_ref(&mut file).take(12).read_to_end(&mut header).map_err(|e| e.to_string())?;
        if Self::from_bytes(&header)?.is_none() {
            return Ok(None);
        }
        if metadata.len() > MAX_IMAGE_BYTES as u64 {
            return Err("image exceeds the 20 MiB upload limit".into());
        }
        file.seek(std::io::SeekFrom::Start(0)).map_err(|e| e.to_string())?;
        let mut bytes = Vec::new();
        file.take(MAX_IMAGE_BYTES as u64 + 1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        Self::from_bytes(&bytes)
    }
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
    #[test]
    fn directory_and_large_text_references_are_not_image_uploads() {
        let dir = tempfile::tempdir().expect("dir");
        assert_eq!(Image::load(dir.path()).expect("directory reference"), None);
        let file = dir.path().join("large.log");
        let mut handle = std::fs::File::create(&file).expect("file");
        std::io::Write::write_all(&mut handle, b"plain text").expect("text");
        handle.set_len(MAX_IMAGE_BYTES as u64 + 1).expect("large text");
        assert_eq!(Image::load(&file).expect("text reference"), None);
    }
}
