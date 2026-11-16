use base64::Engine;
use base64::alphabet::STANDARD;
use base64::engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig};

pub const MAX_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_BASE64: usize = 3_000_000;
pub const TYPES: [&str; 3] = ["image/jpeg", "image/png", "image/webp"];
const PORTRAIT_PARTS: usize = 48;
const PORTRAIT_PART: usize = 24;

const LENIENT: GeneralPurpose = GeneralPurpose::new(
    &STANDARD,
    GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent),
);

pub fn sniff(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return Some("image/jpeg");
    }
    if bytes.starts_with(&[0x89, 0x50, 0x4e, 0x47]) {
        return Some("image/png");
    }
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Some("image/webp");
    }
    None
}

fn without_prefix(raw: &str) -> &str {
    if let Some(rest) = raw.strip_prefix("data:")
        && let Some(at) = rest.find(";base64,")
        && !rest[..at].contains(';')
    {
        return &rest[at + ";base64,".len()..];
    }
    raw
}

pub fn data_url(image_base64: &str, content_type: &str) -> Result<String, String> {
    if image_base64.is_empty() || image_base64.len() > MAX_BASE64 {
        return Err("send the image in base64, up to 2 MB".into());
    }
    let clean: String = without_prefix(image_base64).chars().filter(|c| !c.is_whitespace()).collect();
    let bytes = LENIENT.decode(clean.trim_end_matches('=')).map_err(|_| "The image isn't base64".to_owned())?;
    if bytes.is_empty() {
        return Err("The image is empty".into());
    }
    if bytes.len() > MAX_BYTES {
        return Err("The image can be up to 2 MB".into());
    }
    match sniff(&bytes) {
        Some(found) if found == content_type && TYPES.contains(&content_type) => {
            Ok(format!("data:{found};base64,{}", base64::engine::general_purpose::STANDARD.encode(&bytes)))
        }
        _ => Err("Use a JPEG, PNG or WebP image".into()),
    }
}

pub fn portrait_allowed(code: &str) -> bool {
    let Some(rest) = code.strip_prefix("1.") else { return false };
    let parts: Vec<&str> = rest.split('.').collect();
    parts.len() <= PORTRAIT_PARTS
        && parts
            .iter()
            .all(|part| (1..=PORTRAIT_PART).contains(&part.len()) && part.chars().all(|c| c.is_ascii_alphanumeric()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: [u8; 8] = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];

    fn encoded(bytes: &[u8]) -> String {
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }

    #[test]
    fn images_are_told_apart_by_their_first_bytes() {
        assert_eq!(sniff(&PNG), Some("image/png"));
        assert_eq!(sniff(&[0xff, 0xd8, 0xff, 0xe0]), Some("image/jpeg"));
        assert_eq!(sniff(b"RIFF\0\0\0\0WEBPVP8 "), Some("image/webp"));
        assert_eq!(sniff(b"GIF89a"), None);
    }

    #[test]
    fn an_image_becomes_a_data_url_with_or_without_a_prefix() {
        let plain = data_url(&encoded(&PNG), "image/png");
        assert_eq!(plain, Ok(format!("data:image/png;base64,{}", encoded(&PNG))));
        let prefixed = format!("data:image/png;base64,{}", encoded(&PNG));
        assert_eq!(data_url(&prefixed, "image/png"), plain);
    }

    #[test]
    fn the_declared_type_must_match_the_bytes() {
        assert!(data_url(&encoded(&PNG), "image/jpeg").is_err());
        assert!(data_url(&encoded(b"GIF89a....."), "image/gif").is_err());
    }

    #[test]
    fn empty_broken_or_too_big_images_are_refused() {
        assert!(data_url("", "image/png").is_err());
        assert!(data_url("not base64!", "image/png").is_err());
        let mut big = PNG.to_vec();
        big.resize(MAX_BYTES + 1, 0);
        assert!(data_url(&encoded(&big), "image/png").is_err());
    }

    #[test]
    fn a_portrait_recipe_is_one_and_short_parts() {
        assert!(portrait_allowed("1.f.s3.abc"));
        assert!(!portrait_allowed("1."));
        assert!(!portrait_allowed("2.f"));
        assert!(!portrait_allowed("1.a-b"));
    }
}
