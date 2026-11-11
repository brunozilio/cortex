//! `file://` URIs as language servers write them.

use std::path::{Path, PathBuf};

pub fn uri_of(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    let mut uri = String::from(if text.starts_with('/') { "file://" } else { "file:///" });
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"/-._~:".contains(&byte) {
            uri.push(char::from(byte));
        } else {
            uri.push_str(&format!("%{byte:02X}"));
        }
    }
    uri
}

pub fn path_of(uri: &str) -> Option<PathBuf> {
    let encoded = uri.strip_prefix("file://")?.as_bytes();
    let mut bytes = Vec::with_capacity(encoded.len());
    let mut at = 0;
    while at < encoded.len() {
        let hex = encoded.get(at + 1..at + 3).and_then(|hex| std::str::from_utf8(hex).ok());
        match (encoded[at], hex.and_then(|hex| u8::from_str_radix(hex, 16).ok())) {
            (b'%', Some(byte)) => {
                bytes.push(byte);
                at += 3;
            }
            (byte, _) => {
                bytes.push(byte);
                at += 1;
            }
        }
    }
    String::from_utf8(bytes).ok().map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_with_spaces_and_accents_goes_there_and_back() {
        let path = Path::new("/Users/me/my code/ação#1.rs");
        let uri = uri_of(path);
        assert_eq!(uri, "file:///Users/me/my%20code/a%C3%A7%C3%A3o%231.rs");
        assert_eq!(path_of(&uri).as_deref(), Some(path));
        assert_eq!(path_of("file:///C%3A/x.rs").as_deref(), Some(Path::new("/C:/x.rs")));
        assert_eq!(path_of("https://example.com/a.rs"), None);
    }
}
