//! Narrow service policy: keep navigation protections and component updates.

pub fn switches() -> Vec<(String, Option<String>)> {
    let mut switches = vec![
        ("use-mock-keychain".into(), None),
        ("disable-chrome-login-prompt".into(), None),
        ("disable-sync".into(), None),
        ("disable-domain-reliability".into(), None),
        ("disable-features".into(), Some("OptimizationHints,MediaRouter,AutofillServerCommunication".into())),
    ];
    if cfg!(target_os = "linux") {
        let display =
            ["DISPLAY", "WAYLAND_DISPLAY"].iter().any(|name| std::env::var_os(name).is_some_and(|v| !v.is_empty()));
        switches.extend(linux(display));
    }
    switches
}

pub const ESSENTIAL: [&str; 4] =
    ["use-mock-keychain", "disable-chrome-login-prompt", "password-store", "ozone-platform"];

fn linux(display: bool) -> Vec<(String, Option<String>)> {
    let mut switches = vec![("password-store".into(), Some("basic".into()))];
    if !display {
        switches.push(("ozone-platform".into(), Some("headless".into())));
    }
    switches
}

pub fn through(proxy: std::net::SocketAddr) -> Vec<(String, Option<String>)> {
    vec![
        ("proxy-server".into(), Some(format!("socks5://{proxy}"))),
        ("proxy-bypass-list".into(), Some("<-loopback>".into())),
        ("force-webrtc-ip-handling-policy".into(), Some("disable_non_proxied_udp".into())),
    ]
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_guarded_browser_goes_out_only_through_its_proxy() {
        let switches = super::through("127.0.0.1:4100".parse().expect("address"));
        let value = |name: &str| switches.iter().find(|(n, _)| n == name).and_then(|(_, v)| v.as_deref());
        assert_eq!(value("proxy-server"), Some("socks5://127.0.0.1:4100"), "names resolved by the proxy");
        assert_eq!(value("proxy-bypass-list"), Some("<-loopback>"), "not even loopback goes around it");
        assert_eq!(value("force-webrtc-ip-handling-policy"), Some("disable_non_proxied_udp"));
    }

    #[test]
    fn linux_keeps_the_keyring_out_and_draws_headless_without_a_display() {
        let on_a_desktop = super::linux(true);
        assert_eq!(on_a_desktop, [("password-store".to_owned(), Some("basic".to_owned()))]);
        let on_a_server = super::linux(false);
        assert!(on_a_server.contains(&("ozone-platform".to_owned(), Some("headless".to_owned()))));
        assert!(on_a_server.iter().all(|(name, _)| super::ESSENTIAL.contains(&name.as_str())));
    }

    #[test]
    fn service_policy_preserves_protections_and_component_updates() {
        let switches = super::switches();
        for forbidden in ["disable-background-networking", "disable-component-update", "disable-web-security"] {
            assert!(!switches.iter().any(|(name, _)| name == forbidden));
        }
        assert!(switches.iter().any(|(name, _)| name == "disable-sync"));
        assert!(switches.iter().any(|(name, value)| name == "disable-features"
            && value.as_deref() == Some("OptimizationHints,MediaRouter,AutofillServerCommunication")));
    }
}
