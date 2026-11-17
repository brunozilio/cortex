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
        Ok(address) => is_private_address(address),
        Err(_) => host.chars().all(|c| c.is_ascii_digit() || c == '.') || host.starts_with("0x"),
    }
}

const SPECIAL_V4: [(u32, u32); 16] = [
    (0x0000_0000, 8),
    (0x0a00_0000, 8),
    (0x6440_0000, 10),
    (0x7f00_0000, 8),
    (0xa9fe_0000, 16),
    (0xac10_0000, 12),
    (0xc000_0000, 24),
    (0xc000_0200, 24),
    (0xc058_6300, 24),
    (0xc0a8_0000, 16),
    (0xc612_0000, 15),
    (0xc633_6400, 24),
    (0xcb00_7100, 24),
    (0xe000_0000, 4),
    (0xf000_0000, 4),
    (0xffff_ffff, 32),
];

const SPECIAL_V6: [(u128, u32); 15] = [
    (0, 128),
    (1, 128),
    (0x0064_ff9b_0001_0000_0000_0000_0000_0000, 48),
    (0x0100_0000_0000_0000_0000_0000_0000_0000, 64),
    (0x2001_0000_0000_0000_0000_0000_0000_0000, 32),
    (0x2001_0002_0000_0000_0000_0000_0000_0000, 48),
    (0x2001_0010_0000_0000_0000_0000_0000_0000, 28),
    (0x2001_0020_0000_0000_0000_0000_0000_0000, 28),
    (0x2001_0db8_0000_0000_0000_0000_0000_0000, 32),
    (0x3fff_0000_0000_0000_0000_0000_0000_0000, 20),
    (0x5f00_0000_0000_0000_0000_0000_0000_0000, 16),
    (0xfc00_0000_0000_0000_0000_0000_0000_0000, 7),
    (0xfe80_0000_0000_0000_0000_0000_0000_0000, 10),
    (0xfec0_0000_0000_0000_0000_0000_0000_0000, 10),
    (0xff00_0000_0000_0000_0000_0000_0000_0000, 8),
];

const EMBEDDING_V6: [(u128, u32); 4] = [
    (0, 96),
    (0x0000_0000_0000_0000_0000_ffff_0000_0000, 96),
    (0x0000_0000_0000_0000_ffff_0000_0000_0000, 96),
    (0x0064_ff9b_0000_0000_0000_0000_0000_0000, 96),
];

const SIX_TO_FOUR: u128 = 0x2002_0000_0000_0000_0000_0000_0000_0000;

fn within<const BITS: u32>(address: u128, (network, length): (u128, u32)) -> bool {
    let shift = BITS - length;
    length == 0 || (address >> shift) == (network >> shift)
}

pub fn is_private_address(address: std::net::IpAddr) -> bool {
    match address {
        std::net::IpAddr::V4(ip) => {
            let bits = u128::from(u32::from(ip));
            SPECIAL_V4.iter().any(|&(network, length)| within::<32>(bits, (u128::from(network), length)))
        }
        std::net::IpAddr::V6(ip) => {
            let bits = u128::from(ip);
            if SPECIAL_V6.iter().any(|&block| within::<128>(bits, block)) {
                return true;
            }
            let low = |bits: u128| std::net::Ipv4Addr::from((bits & 0xffff_ffff) as u32);
            let embedded = if EMBEDDING_V6.iter().any(|&block| within::<128>(bits, block)) {
                Some(low(bits))
            } else if within::<128>(bits, (SIX_TO_FOUR, 16)) {
                Some(low(bits >> 80))
            } else {
                None
            };
            embedded.is_some_and(|v4| is_private_address(v4.into()))
        }
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

    #[test]
    fn a_resolved_address_is_judged_like_a_written_one() {
        for address in
            ["127.0.0.1", "169.254.169.254", "100.100.100.200", "::ffff:10.0.0.1", "64:ff9b::7f00:1", "fd00::1"]
        {
            assert!(is_private_address(address.parse().expect("address")), "{address}");
        }
        for address in ["93.184.216.34", "2606:2800:220:1:248:1893:25c8:1946", "100.128.0.1"] {
            assert!(!is_private_address(address.parse().expect("address")), "{address}");
        }
    }

    #[test]
    fn every_special_purpose_block_of_the_iana_registries_is_private_and_public_neighbours_are_not() {
        let special = [
            ("0.0.0.0", "this network"),
            ("0.1.2.3", "this network, 0.0.0.0/8"),
            ("10.255.255.255", "private, 10/8"),
            ("100.64.0.1", "shared address space, 100.64/10"),
            ("100.127.255.254", "shared address space, top"),
            ("127.1.2.3", "loopback, 127/8"),
            ("169.254.0.1", "link local"),
            ("172.16.0.1", "private, 172.16/12"),
            ("172.31.255.255", "private, 172.16/12 top"),
            ("192.0.0.8", "IETF protocol assignments, 192.0.0/24"),
            ("192.0.0.170", "NAT64 discovery, 192.0.0/24"),
            ("192.0.2.1", "documentation, TEST-NET-1"),
            ("192.88.99.1", "6to4 relay anycast"),
            ("192.168.0.1", "private, 192.168/16"),
            ("198.18.0.1", "benchmarking, 198.18/15"),
            ("198.19.255.254", "benchmarking, top"),
            ("198.51.100.7", "documentation, TEST-NET-2"),
            ("203.0.113.9", "documentation, TEST-NET-3"),
            ("224.0.0.1", "multicast, 224/4"),
            ("239.255.255.250", "multicast, top"),
            ("240.0.0.1", "reserved, 240/4"),
            ("255.255.255.254", "reserved, 240/4 top"),
            ("255.255.255.255", "limited broadcast"),
            ("::", "unspecified, ::/128"),
            ("::1", "loopback, ::1/128"),
            ("::127.0.0.1", "IPv4-compatible loopback, ::/96"),
            ("::10.0.0.1", "IPv4-compatible private, ::/96"),
            ("::a9fe:a9fe", "IPv4-compatible metadata, ::/96"),
            ("::ffff:127.0.0.1", "IPv4-mapped loopback, ::ffff:0:0/96"),
            ("::ffff:169.254.169.254", "IPv4-mapped metadata"),
            ("::ffff:0:10.0.0.1", "IPv4-translated, ::ffff:0:0:0/96"),
            ("64:ff9b::10.0.0.1", "NAT64 of a private target, 64:ff9b::/96"),
            ("64:ff9b::a9fe:a9fe", "NAT64 of the metadata address"),
            ("64:ff9b:1::1", "local-use NAT64, 64:ff9b:1::/48"),
            ("100::1", "discard only, 100::/64"),
            ("2001::1", "Teredo, 2001::/32"),
            ("2001:2::1", "benchmarking, 2001:2::/48"),
            ("2001:10::1", "ORCHID, 2001:10::/28"),
            ("2001:db8::1", "documentation, 2001:db8::/32"),
            ("2002:7f00:1::1", "6to4 of loopback, 2002::/16"),
            ("2002:a00:1::1", "6to4 of a private address"),
            ("3fff::1", "documentation, 3fff::/20"),
            ("5f00::1", "segment routing, 5f00::/16"),
            ("fc00::1", "unique local, fc00::/7"),
            ("fdff:ffff::1", "unique local, top"),
            ("fe80::1", "link local, fe80::/10"),
            ("febf::1", "link local, top"),
            ("fec0::1", "site local, fec0::/10"),
            ("ff02::1", "multicast, ff00::/8"),
            ("ff0e::1", "global multicast, ff00::/8"),
        ];
        for (address, block) in special {
            let parsed: std::net::IpAddr = address.parse().expect("address");
            assert!(is_private_address(parsed), "{address} ({block}) is reachable");
            assert!(is_private(address), "{address} ({block}) as a written host");
        }
        let public = [
            "1.1.1.1",
            "8.8.8.8",
            "100.63.255.255",
            "100.128.0.0",
            "172.15.255.255",
            "172.32.0.0",
            "192.0.1.1",
            "192.0.3.1",
            "192.169.0.1",
            "198.17.255.255",
            "198.20.0.0",
            "198.51.99.1",
            "203.0.112.1",
            "223.255.255.255",
            "2606:4700:4700::1111",
            "2a00:1450:4001::200e",
            "64:ff9b::808:808",
            "::ffff:8.8.8.8",
            "2002:808:808::1",
            "2001:4860:4860::8888",
        ];
        for address in public {
            assert!(!is_private_address(address.parse().expect("address")), "{address} is refused");
        }
    }
}
