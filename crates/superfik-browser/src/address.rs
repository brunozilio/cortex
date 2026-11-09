//! The address bar: what was typed → the URL to load; and where a page
//! on the server may go ([`server_may_open`]). Pure.

mod reach;

pub use reach::{Refusal, is_public, server_may_open};

pub const SEARCH: &str = "https://www.google.com/search?q=";

pub fn resolve(typed: &str) -> Option<String> {
    let typed = typed.trim();
    if typed.is_empty() {
        return None;
    }
    if has_scheme(typed) {
        return Some(typed.to_owned());
    }
    if typed.contains(char::is_whitespace) {
        return Some(search(typed));
    }
    let host = typed.split(['/', '?', '#']).next().unwrap_or(typed);
    let name = host.rsplit_once(':').filter(|(_, port)| is_port(port)).map_or(host, |(name, _)| name);
    if is_local(name) {
        return Some(format!("http://{typed}"));
    }
    if looks_like_domain(name) {
        return Some(format!("https://{typed}"));
    }
    Some(search(typed))
}

fn has_scheme(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    ["http://", "https://", "file://", "about:", "data:", "chrome://", "view-source:"]
        .iter()
        .any(|scheme| lower.starts_with(scheme))
}

fn is_port(text: &str) -> bool {
    !text.is_empty() && text.len() <= 5 && text.bytes().all(|b| b.is_ascii_digit())
}

fn is_local(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower == "localhost" || lower.ends_with(".localhost") || lower.ends_with(".local") || is_ipv4(name)
}

fn is_ipv4(name: &str) -> bool {
    let parts: Vec<&str> = name.split('.').collect();
    parts.len() == 4 && parts.iter().all(|p| !p.is_empty() && p.len() <= 3 && p.parse::<u8>().is_ok())
}

fn looks_like_domain(name: &str) -> bool {
    let Some((rest, tld)) = name.rsplit_once('.') else { return false };
    let label = |l: &str| !l.is_empty() && l.chars().all(|c| c.is_alphanumeric() || c == '-');
    tld.len() >= 2 && tld.chars().all(char::is_alphabetic) && rest.split('.').all(label)
}

fn search(words: &str) -> String {
    let mut query = String::with_capacity(words.len());
    for byte in words.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => query.push(byte as char),
            b' ' => query.push('+'),
            _ => query.push_str(&format!("%{byte:02X}")),
        }
    }
    format!("{SEARCH}{query}")
}

pub fn display(url: &str) -> &str {
    let bare = url.strip_prefix("https://").unwrap_or(url);
    bare.strip_suffix('/').filter(|b| !b.contains('/')).unwrap_or(bare)
}

pub fn site(url: &str) -> &str {
    let Some(rest) = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://")) else {
        return display(url);
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    host.strip_prefix("www.").unwrap_or(host)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_site_is_the_host_without_www() {
        assert_eq!(site("https://www.linkedin.com/notifications/"), "linkedin.com");
        assert_eq!(site("http://localhost:3000/app?x=1"), "localhost:3000");
        assert_eq!(site("https://gemini.google.com/app?hl=pt-BR"), "gemini.google.com");
        assert_eq!(site("about:blank"), "about:blank");
        assert_eq!(site(""), "");
    }

    #[test]
    fn schemes_are_kept() {
        assert_eq!(resolve(" https://a.dev/x ").as_deref(), Some("https://a.dev/x"));
        assert_eq!(resolve("about:blank").as_deref(), Some("about:blank"));
        assert_eq!(resolve("file:///tmp/a.html").as_deref(), Some("file:///tmp/a.html"));
    }

    #[test]
    fn local_hosts_use_http() {
        assert_eq!(resolve("localhost:3000").as_deref(), Some("http://localhost:3000"));
        assert_eq!(resolve("127.0.0.1:8080/api").as_deref(), Some("http://127.0.0.1:8080/api"));
        assert_eq!(resolve("app.localhost").as_deref(), Some("http://app.localhost"));
    }

    #[test]
    fn domains_use_https() {
        assert_eq!(resolve("example.com").as_deref(), Some("https://example.com"));
        assert_eq!(resolve("docs.rs/wgpu?x=1").as_deref(), Some("https://docs.rs/wgpu?x=1"));
    }

    #[test]
    fn words_are_searched() {
        assert_eq!(resolve("rust wgpu").as_deref(), Some("https://www.google.com/search?q=rust+wgpu"));
        assert_eq!(resolve("c++").as_deref(), Some("https://www.google.com/search?q=c%2B%2B"));
        assert_eq!(resolve("file.rs1").as_deref(), Some("https://www.google.com/search?q=file.rs1"));
        assert_eq!(resolve("   "), None);
    }

    #[test]
    fn display_drops_the_https_scheme_and_a_bare_trailing_slash() {
        assert_eq!(display("https://example.com/"), "example.com");
        assert_eq!(display("https://example.com/a/"), "example.com/a/");
        assert_eq!(display("http://localhost:3000/"), "http://localhost:3000/");
    }
}
