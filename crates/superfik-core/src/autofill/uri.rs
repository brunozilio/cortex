//! Which saved logins belong to a page: each saved URI and its match rule,
//! as Bitwarden compares them (`LoginUriView::matchesUri`).

use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UriRule {
    #[default]
    Domain,
    Host,
    StartsWith,
    Exact,
    RegularExpression,
    Never,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoginUri {
    pub uri: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<UriRule>,
}

impl LoginUri {
    pub fn new(uri: impl Into<String>, rule: Option<UriRule>) -> Self {
        Self { uri: uri.into(), rule }
    }

    pub fn matches(&self, page: &str) -> bool {
        let (saved, page) = (self.uri.trim(), page.trim());
        if saved.is_empty() || page.is_empty() {
            return false;
        }
        match self.rule.unwrap_or_default() {
            UriRule::Domain => match (domain(saved), domain(page)) {
                (Some(saved), Some(page)) => super::equivalent::equivalent(&saved, &page),
                _ => false,
            },
            UriRule::Host => host(saved).is_some_and(|h| host(page).as_ref() == Some(&h)),
            UriRule::StartsWith => page.starts_with(saved),
            UriRule::Exact => page == saved,
            UriRule::RegularExpression => {
                regex::RegexBuilder::new(saved).case_insensitive(true).build().is_ok_and(|re| re.is_match(page))
            }
            UriRule::Never => false,
        }
    }
}

pub fn matches_any(uris: &[LoginUri], page: &str) -> bool {
    uris.iter().any(|uri| uri.matches(page))
}

pub fn same_document(chosen: &str, now: &str) -> bool {
    let (Ok(chosen), Ok(now)) = (Url::parse(chosen.trim()), Url::parse(now.trim())) else { return false };
    chosen.origin().is_tuple() && chosen.origin() == now.origin() && chosen.path() == now.path()
}

fn parse(text: &str) -> Option<Url> {
    if text.contains("://") {
        Url::parse(text).ok()
    } else if text.contains('.') {
        Url::parse(&format!("http://{text}")).ok()
    } else {
        None
    }
}

fn host(text: &str) -> Option<String> {
    let url = parse(text)?;
    let host = url.host_str()?.to_ascii_lowercase();
    Some(match url.port() {
        Some(port) => format!("{host}:{port}"),
        None => host,
    })
}

fn domain(text: &str) -> Option<String> {
    let url = parse(text)?;
    match url.host()? {
        url::Host::Domain(name) => {
            let name = name.trim_end_matches('.').to_ascii_lowercase();
            if name == "localhost" {
                return Some(name);
            }
            let registrable = psl::domain(name.as_bytes())?;
            String::from_utf8(registrable.as_bytes().to_vec()).ok()
        }
        url::Host::Ipv4(ip) => Some(ip.to_string()),
        url::Host::Ipv6(ip) => Some(ip.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn saved(uri: &str, rule: UriRule) -> LoginUri {
        LoginUri::new(uri, Some(rule))
    }

    #[test]
    fn domain_is_the_default_and_takes_subdomains_schemes_and_paths() {
        let uri = LoginUri::new("https://github.com/login", None);
        assert!(uri.matches("https://github.com/session"));
        assert!(uri.matches("https://gist.github.com/"));
        assert!(uri.matches("http://github.com:8443/x"));
        assert!(!uri.matches("https://github.community/"));
        assert!(!uri.matches("https://notgithub.com/"));
    }

    #[test]
    fn domain_reads_a_saved_address_without_a_scheme() {
        assert!(LoginUri::new("accounts.google.com", None).matches("https://mail.google.com/"));
        assert!(!LoginUri::new("intranet", None).matches("https://intranet/"));
    }

    #[test]
    fn domain_takes_equivalent_domains_and_host_does_not() {
        assert!(LoginUri::new("https://accounts.google.com", None).matches("https://www.youtube.com/signin"));
        assert!(LoginUri::new("https://www.amazon.com", None).matches("https://www.amazon.com.br/ap/signin"));
        assert!(!saved("https://accounts.google.com", UriRule::Host).matches("https://www.youtube.com/"));
    }

    #[test]
    fn domain_keeps_private_suffixes_apart() {
        let uri = saved("https://alice.github.io", UriRule::Domain);
        assert!(uri.matches("https://alice.github.io/blog"));
        assert!(!uri.matches("https://mallory.github.io/"));
    }

    #[test]
    fn domain_knows_multi_part_suffixes() {
        let uri = saved("https://www.itau.com.br", UriRule::Domain);
        assert!(uri.matches("https://internet.itau.com.br/"));
        assert!(!uri.matches("https://bradesco.com.br/"));
    }

    #[test]
    fn domain_compares_localhost_and_ips_whole() {
        assert!(saved("http://localhost:3000", UriRule::Domain).matches("http://localhost:5173/login"));
        assert!(saved("http://10.0.0.5", UriRule::Domain).matches("https://10.0.0.5:8443/"));
        assert!(!saved("http://10.0.0.5", UriRule::Domain).matches("http://10.0.0.6/"));
    }

    #[test]
    fn host_needs_the_same_host_and_port() {
        let uri = saved("https://app.example.com:8443/login", UriRule::Host);
        assert!(uri.matches("https://app.example.com:8443/home"));
        assert!(!uri.matches("https://app.example.com/home"));
        assert!(!uri.matches("https://example.com:8443/"));
    }

    #[test]
    fn starts_with_and_exact_compare_the_text() {
        assert!(saved("https://example.com/admin", UriRule::StartsWith).matches("https://example.com/admin/users"));
        assert!(!saved("https://example.com/admin", UriRule::StartsWith).matches("https://example.com/"));
        assert!(saved("https://example.com/a", UriRule::Exact).matches("https://example.com/a"));
        assert!(!saved("https://example.com/a", UriRule::Exact).matches("https://example.com/a?x=1"));
    }

    #[test]
    fn regular_expressions_ignore_case_and_an_invalid_one_never_matches() {
        assert!(saved(r"^https://(www\.)?example\.com/", UriRule::RegularExpression).matches("HTTPS://EXAMPLE.COM/"));
        assert!(!saved("(", UriRule::RegularExpression).matches("https://example.com/"));
    }

    #[test]
    fn never_and_empty_addresses_never_match() {
        assert!(!saved("https://example.com", UriRule::Never).matches("https://example.com"));
        assert!(!LoginUri::new("", None).matches("https://example.com"));
        assert!(!LoginUri::new("https://example.com", None).matches(""));
        assert!(!LoginUri::new("https://example.com", None).matches("about:blank"));
    }

    #[test]
    fn the_same_document_keeps_origin_and_path() {
        assert!(same_document("https://a.com/login?next=/", "https://a.com/login#step2"));
        assert!(!same_document("https://a.com/login", "https://a.com/other"));
        assert!(!same_document("https://a.com/login", "http://a.com/login"));
        assert!(!same_document("https://a.com/login", "https://evil.com/login"));
        assert!(!same_document("about:blank", "about:blank"));
    }

    #[test]
    fn any_saved_address_will_do() {
        let uris = [LoginUri::new("https://a.com", None), LoginUri::new("https://b.com", None)];
        assert!(matches_any(&uris, "https://b.com/login"));
        assert!(!matches_any(&uris, "https://c.com/login"));
    }
}
