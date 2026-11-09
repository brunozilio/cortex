//! A profile's one-line summary under its name: what it changes. Pure.

use superfik_core::browser_profiles::Laboratory;
use superfik_i18n::{Strings, Text};

pub fn summary(settings: &Laboratory, strings: &Strings) -> String {
    let mut parts = Vec::new();
    if let Some(cpu) = settings.hardware_concurrency {
        parts.push(format!("{cpu} CPU"));
    }
    if let Some(memory) = settings.device_memory {
        parts.push(format!("{memory} GiB"));
    }
    if let Some(screen) = settings.screen {
        parts.push(format!("{}×{}", screen.width, screen.height));
    }
    if let Some(gpu) = settings.gpu.as_ref().filter(|gpu| !gpu.vendor.is_empty()) {
        parts.push(gpu.vendor.clone());
    }
    if let Some(proxy) = &settings.proxy {
        parts.push(format!("{}:{}", proxy.host, proxy.port));
    }
    if parts.is_empty() {
        parts.push(strings.t(Text::BrowserProfileAllActual));
    }
    if !settings.enabled {
        parts.insert(0, strings.t(Text::BrowserProfileOff));
    }
    parts.join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use superfik_core::browser_profiles::{Proxy, ProxyScheme, Screen};
    use superfik_i18n::Language;

    fn english() -> Strings {
        Strings::new(Language::English)
    }

    #[test]
    fn lists_what_the_profile_changes() {
        let settings = Laboratory {
            enabled: true,
            hardware_concurrency: Some(4),
            device_memory: Some(0.5),
            screen: Some(Screen { width: 1920, height: 1080, scale: 2., depth: 24 }),
            proxy: Some(Proxy { scheme: ProxyScheme::Http, host: "proxy.lab".into(), port: 8080, username: None }),
            ..Laboratory::default()
        };
        assert_eq!(summary(&settings, &english()), "4 CPU · 0.5 GiB · 1920×1080 · proxy.lab:8080");
    }

    #[test]
    fn says_when_it_is_off_or_changes_nothing() {
        let settings = Laboratory::default();
        assert_eq!(summary(&settings, &english()), "Off · Nothing changed yet");
        let on = Laboratory { enabled: true, ..settings };
        assert_eq!(summary(&on, &english()), "Nothing changed yet");
    }
}
