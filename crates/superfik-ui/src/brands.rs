//! Official marks of the services plugins connect (sources and licences in
//! `brands/README.md`), preserving the artwork's colours and proportions.
//! Unknown services get a monogram.

use superfik_render::{Color, Icon};

macro_rules! brand {
    ($name:ident, $file:literal) => {
        pub const $name: Icon = Icon::new(concat!("brand-", $file), include_str!(concat!("brands/", $file, ".svg")));
    };
}

brand!(GITHUB, "github");
brand!(LINEAR, "linear");
brand!(NOTION, "notion");
brand!(SENTRY, "sentry");
brand!(STRIPE, "stripe");
brand!(POSTGRESQL, "postgresql");
brand!(FIRECRAWL, "firecrawl");
brand!(PLAYWRIGHT, "playwright");
brand!(BITWARDEN, "bitwarden");
brand!(WHATSAPP, "whatsapp");
brand!(CLAUDE, "claude");
brand!(OPENAI, "openai");
brand!(ANTHROPIC, "anthropic");
brand!(CONTEXT7, "context7");
brand!(ATLASSIAN, "atlassian");
brand!(INTERCOM, "intercom");
brand!(INSTAGRAM, "instagram");
brand!(X, "x");
brand!(LINKEDIN, "linkedin");

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Brand {
    pub mark: Option<Icon>,
    pub color: Color,
    pub own_colours: bool,
    pub monogram: Option<&'static str>,
}

const NEUTRAL: Color = Color::hex(0x5b5f66);

pub fn brand(key: &str) -> Brand {
    let plain =
        |mark: Option<Icon>, color: u32| Brand { mark, color: Color::hex(color), own_colours: true, monogram: None };
    let (mark, color) = match key {
        "claude" => (Some(CLAUDE), 0xc96442),
        "anthropic" => (Some(ANTHROPIC), 0x141413),
        "openai" | "chatgpt" => (Some(OPENAI), 0x171717),
        "github" => (Some(GITHUB), 0x24292f),
        "linear" => (Some(LINEAR), 0x5e6ad2),
        "notion" => (Some(NOTION), 0x191919),
        "sentry" => (Some(SENTRY), 0x362d59),
        "stripe" => (Some(STRIPE), 0x635bff),
        "postgresql" => (Some(POSTGRESQL), 0x4169e1),
        "firecrawl" => (Some(FIRECRAWL), 0xfa5d19),
        "bitwarden" => (Some(BITWARDEN), 0x175ddc),
        "whatsapp" => (Some(WHATSAPP), 0x25d366),
        "playwright" => (Some(PLAYWRIGHT), 0xf4f5f7),
        "context7" => (Some(CONTEXT7), 0x171717),
        "atlassian" => (Some(ATLASSIAN), 0x0052cc),
        "intercom" => (Some(INTERCOM), 0x171717),
        "email" => return Brand { monogram: Some("@"), own_colours: false, ..plain(None, 0x4b5563) },
        "instagram" => (Some(INSTAGRAM), 0xe4405f),
        "x" => (Some(X), 0x000000),
        "linkedin" => (Some(LINKEDIN), 0x0a66c2),
        _ => return Brand { mark: None, color: NEUTRAL, own_colours: false, monogram: None },
    };
    plain(mark, color)
}

pub const ALL: [Icon; 19] = [
    GITHUB, LINEAR, NOTION, SENTRY, STRIPE, POSTGRESQL, FIRECRAWL, PLAYWRIGHT, BITWARDEN, WHATSAPP, CLAUDE, OPENAI,
    ANTHROPIC, CONTEXT7, ATLASSIAN, INTERCOM, INSTAGRAM, X, LINKEDIN,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_rasterize_and_unknown_keys_fall_back() {
        for icon in ALL {
            for size in [24, 80] {
                let pixels =
                    superfik_render::icons::rasterize(&icon, size, size).unwrap_or_else(|| panic!("{}", icon.name));
                assert!(
                    pixels.chunks(4).any(|p| p[3] > 0 && p[..3].iter().any(|channel| *channel < 230)),
                    "{} has no visible artwork",
                    icon.name
                );
                assert_eq!(pixels[3], 0, "{} must clip its corners", icon.name);
                let middle_of_top = (size as usize / 2) * 4 + 3;
                assert!(pixels[middle_of_top] > 0, "{} must fill its tile", icon.name);
                let mut low = [255u8; 3];
                let mut high = [0u8; 3];
                for pixel in pixels.as_chunks::<4>().0.iter().filter(|pixel| pixel[3] == 255) {
                    for channel in 0..3 {
                        low[channel] = low[channel].min(pixel[channel]);
                        high[channel] = high[channel].max(pixel[channel]);
                    }
                }
                assert!(
                    (0..3).any(|channel| high[channel].saturating_sub(low[channel]) > 64),
                    "{} must show a contrasting mark, not just a flat background",
                    icon.name
                );
            }
        }
        assert_eq!(brand("linear").mark.map(|m| m.name), Some("brand-linear"));
        assert!(brand("playwright").own_colours);
        assert_eq!(brand("context7").mark, Some(CONTEXT7));
        assert_ne!(brand("claude").mark, brand("anthropic").mark);
        assert_eq!(brand("whatever").color, NEUTRAL);
    }

    #[test]
    fn catalog_services_use_untinted_artwork() {
        let sources: serde_json::Value =
            serde_json::from_str(include_str!("brands/sources.json")).expect("sources.json parses");
        for key in sources.as_object().expect("sources.json is an object").keys() {
            let service = brand(key);
            assert!(service.mark.is_some(), "{key}");
            assert!(service.own_colours, "{key} must preserve its colours");
            assert!(service.monogram.is_none(), "{key}");
        }
    }
}
