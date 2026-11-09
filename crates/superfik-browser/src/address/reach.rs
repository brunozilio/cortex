//! Where a page running on the server may go. Pure.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    Scheme,
    Address,
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Scheme => "pages on the server open only http and https addresses",
            Self::Address => "pages on the server cannot open the server's own machine or network",
        })
    }
}

pub fn server_may_open(url: &str) -> Result<(), Refusal> {
    let Ok(parsed) = url::Url::parse(url.trim()) else { return Err(Refusal::Scheme) };
    if parsed.scheme() == "about" && parsed.path() == "blank" {
        return Ok(());
    }
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(Refusal::Scheme);
    }
    let allowed = match parsed.host() {
        Some(url::Host::Domain(name)) => {
            let name = name.trim_end_matches('.').to_ascii_lowercase();
            name != "localhost" && !name.ends_with(".localhost")
        }
        Some(url::Host::Ipv4(ip)) => is_public(ip.into()),
        Some(url::Host::Ipv6(ip)) => is_public(ip.into()),
        None => false,
    };
    allowed.then_some(()).ok_or(Refusal::Address)
}

pub fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_public_v4(ip),
        IpAddr::V6(ip) => is_public_v6(ip),
    }
}

fn is_public_v4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    let reserved = a == 0
        || a == 10
        || a == 127
        || (a == 100 && (64..128).contains(&b))
        || (a == 169 && b == 254)
        || (a == 172 && (16..32).contains(&b))
        || (a == 192 && b == 0 && c == 0)
        || (a == 192 && b == 0 && c == 2)
        || (a == 192 && b == 168)
        || (a == 198 && (18..20).contains(&b))
        || (a == 198 && b == 51 && c == 100)
        || (a == 203 && b == 0 && c == 113)
        || a >= 224;
    !reserved
}

fn v4(high: u16, low: u16) -> Ipv4Addr {
    let [a, b] = high.to_be_bytes();
    let [c, d] = low.to_be_bytes();
    Ipv4Addr::new(a, b, c, d)
}

fn is_public_v6(ip: Ipv6Addr) -> bool {
    if let Some(mapped) = ip.to_ipv4_mapped() {
        return is_public_v4(mapped);
    }
    let s = ip.segments();
    if s[0] == 0x64 && s[1] == 0xff9b && s[2..6] == [0, 0, 0, 0] {
        return is_public_v4(v4(s[6], s[7]));
    }
    if s[0] == 0x2002 {
        return is_public_v4(v4(s[1], s[2]));
    }
    let reserved = s[0..6] == [0, 0, 0, 0, 0, 0]
        || (s[0] & 0xfe00) == 0xfc00
        || (s[0] & 0xffc0) == 0xfe80
        || (s[0] & 0xffc0) == 0xfec0
        || (s[0] & 0xff00) == 0xff00
        || (s[0] == 0x2001 && s[1] == 0x0db8)
        || (s[0] == 0x2001 && s[1] == 0)
        || (s[0] == 0x0100 && s[1..4] == [0, 0, 0]);
    !reserved
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_server_page_opens_only_the_public_web() {
        for open in [
            "https://example.com/a?b",
            "http://example.com:8080/",
            "HTTPS://Example.COM",
            "https://93.184.216.34/",
            "https://[2606:2800:220:1:248:1893:25c8:1946]/",
            "about:blank",
        ] {
            assert_eq!(server_may_open(open), Ok(()), "{open}");
        }
        for (closed, why) in [
            ("file:///etc/passwd", Refusal::Scheme),
            ("view-source:https://example.com", Refusal::Scheme),
            ("chrome://settings", Refusal::Scheme),
            ("devtools://devtools/bundled/inspector.html", Refusal::Scheme),
            ("data:text/html,<h1>x</h1>", Refusal::Scheme),
            ("javascript:alert(1)", Refusal::Scheme),
            ("ftp://example.com", Refusal::Scheme),
            ("about:settings", Refusal::Scheme),
            ("http://localhost:3000", Refusal::Address),
            ("http://app.localhost/", Refusal::Address),
            ("http://127.0.0.1:7420/", Refusal::Address),
            ("http://2130706433/", Refusal::Address),
            ("http://0x7f.1/", Refusal::Address),
            ("http://169.254.169.254/latest/meta-data", Refusal::Address),
            ("http://10.0.0.5/", Refusal::Address),
            ("http://172.16.3.4/", Refusal::Address),
            ("http://192.168.1.1/", Refusal::Address),
            ("http://0.0.0.0/", Refusal::Address),
            ("http://[::1]/", Refusal::Address),
            ("http://[fd00::1]/", Refusal::Address),
            ("http://[fe80::1]/", Refusal::Address),
            ("http://[::ffff:127.0.0.1]/", Refusal::Address),
            ("not a url", Refusal::Scheme),
        ] {
            assert_eq!(server_may_open(closed), Err(why), "{closed}");
        }
    }

    #[test]
    fn public_addresses_are_the_internets_own() {
        let public = |ip: &str| is_public(ip.parse().expect("ip"));
        for ip in ["93.184.216.34", "8.8.8.8", "2606:4700:4700::1111", "1.1.1.1"] {
            assert!(public(ip), "{ip}");
        }
        for ip in [
            "127.0.0.1",
            "127.1.2.3",
            "10.1.2.3",
            "172.31.255.255",
            "192.168.0.1",
            "169.254.169.254",
            "100.64.0.1",
            "0.0.0.0",
            "0.1.2.3",
            "255.255.255.255",
            "224.0.0.1",
            "192.0.2.1",
            "198.18.0.1",
            "240.0.0.1",
            "::",
            "::1",
            "fc00::1",
            "fd12:3456::1",
            "fe80::1",
            "fec0::1",
            "ff02::1",
            "::ffff:10.0.0.1",
            "::ffff:169.254.169.254",
            "64:ff9b::a9fe:a9fe",
            "2002:7f00:1::",
            "2001:db8::1",
        ] {
            assert!(!public(ip), "{ip}");
        }
    }
}
