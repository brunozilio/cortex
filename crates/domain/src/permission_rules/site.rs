pub fn site(url: &str) -> Option<String> {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let host = rest.split(['/', '?', '#', '\\']).next()?;
    let host = host.rsplit_once('@').map_or(host, |(_, host)| host);
    let host = match host.strip_prefix('[') {
        Some(v6) => v6.split(']').next()?,
        None => host.split(':').next()?,
    };
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    (!host.is_empty()).then_some(host)
}

pub fn is_private(host: &str) -> bool {
    if host == "localhost" || host.ends_with(".localhost") || host.ends_with(".local") || host.ends_with(".internal") {
        return true;
    }
    match host.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(ip)) => {
            ip.is_loopback() || ip.is_private() || ip.is_link_local() || ip.is_unspecified() || ip.octets()[0] == 0
        }
        Ok(std::net::IpAddr::V6(ip)) => {
            let first = ip.segments()[0];
            ip.is_loopback()
                || ip.is_unspecified()
                || (first & 0xfe00) == 0xfc00
                || (first & 0xffc0) == 0xfe80
                || ip.to_ipv4_mapped().is_some_and(|v4| is_private(&v4.to_string()))
        }
        Err(_) => host.chars().all(|c| c.is_ascii_digit() || c == '.') || host.starts_with("0x"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_site_is_named_one_way() {
        assert_eq!(site("https://Docs.RS./serde?x=1").as_deref(), Some("docs.rs"));
        assert_eq!(site("https://user@example.com:8080/a").as_deref(), Some("example.com"));
        assert_eq!(site("https://[::1]:80/").as_deref(), Some("::1"));
        assert_eq!(site("https://"), None);
    }

    #[test]
    fn this_computer_and_its_network_are_private() {
        for host in ["localhost", "127.0.0.1", "10.0.0.8", "192.168.1.1", "172.20.0.1", "169.254.169.254", "::1"] {
            assert!(is_private(host), "{host}");
        }
        assert!(is_private("2130706433"), "127.0.0.1 written as a number");
        assert!(is_private("metadata.google.internal"));
        for host in ["docs.rs", "8.8.8.8", "172.32.0.1"] {
            assert!(!is_private(host), "{host}");
        }
    }
}
