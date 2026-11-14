use std::cell::Cell;

use ratatui::style::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Background {
    Dark,
    Light,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub background: Background,
    pub truecolor: bool,
}

thread_local! {
    static CURRENT: Cell<Option<Theme>> = const { Cell::new(None) };
}

impl Theme {
    pub fn detect(var: impl Fn(&str) -> Option<String>) -> Self {
        let chosen = var(concat!(brand::env_prefix!(), "_THEME")).map(|v| v.to_ascii_lowercase());
        let background = match chosen.as_deref() {
            Some("light") => Background::Light,
            Some("dark") => Background::Dark,
            _ => var("COLORFGBG").map_or(Background::Dark, |pair| from_colorfgbg(&pair)),
        };
        let truecolor = var("COLORTERM").is_some_and(|v| matches!(v.as_str(), "truecolor" | "24bit"));
        Self { background, truecolor }
    }

    pub fn light(self) -> bool {
        self.background == Background::Light
    }

    fn pick(self, dark: (u8, u8, u8, u8), light: (u8, u8, u8, u8)) -> Color {
        let (r, g, b, indexed) = if self.light() { light } else { dark };
        if self.truecolor { Color::Rgb(r, g, b) } else { Color::Indexed(indexed) }
    }

    fn tint(self, dark: (u8, u8, u8), light: (u8, u8, u8, u8)) -> Color {
        match (self.truecolor, self.light()) {
            (false, false) => Color::Reset,
            _ => self.pick((dark.0, dark.1, dark.2, 0), light),
        }
    }

    pub fn added_bg(self) -> Color {
        self.tint((0x1d, 0x3b, 0x2a), (0xda, 0xfb, 0xe1, 194))
    }

    pub fn removed_bg(self) -> Color {
        self.tint((0x45, 0x22, 0x28), (0xff, 0xeb, 0xe9, 224))
    }

    pub fn added_mark(self) -> Color {
        self.pick((0x7e, 0xe7, 0x87, 114), (0x1a, 0x7f, 0x37, 28))
    }

    pub fn removed_mark(self) -> Color {
        self.pick((0xff, 0x8a, 0x8a, 210), (0xcf, 0x22, 0x2e, 160))
    }

    pub fn selection_bg(self) -> Color {
        self.pick((0x26, 0x2b, 0x36, 236), (0xe4, 0xe9, 0xf2, 254))
    }

    pub fn code_bg(self) -> Color {
        self.pick((0x1f, 0x22, 0x29, 235), (0xf1, 0xf3, 0xf6, 255))
    }

    pub fn banded(self) -> bool {
        self.truecolor || self.light()
    }

    pub fn code_theme(self) -> &'static str {
        if self.light() { "InspiredGitHub" } else { "base16-ocean.dark" }
    }
}

fn from_colorfgbg(pair: &str) -> Background {
    match pair.rsplit(';').next().and_then(|bg| bg.parse::<u8>().ok()) {
        Some(7 | 9..=15) => Background::Light,
        _ => Background::Dark,
    }
}

pub fn current() -> Theme {
    CURRENT.with(|cell| {
        cell.get().unwrap_or_else(|| {
            let theme = Theme::detect(|name| std::env::var(name).ok());
            cell.set(Some(theme));
            theme
        })
    })
}

#[cfg(test)]
pub fn set(theme: Theme) {
    CURRENT.with(|cell| cell.set(Some(theme)));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detect(vars: &[(&str, &str)]) -> Theme {
        Theme::detect(|name| vars.iter().find(|(key, _)| *key == name).map(|(_, value)| (*value).to_owned()))
    }

    #[test]
    fn the_background_comes_from_the_override_then_colorfgbg_then_defaults_to_dark() {
        let override_name = concat!(brand::env_prefix!(), "_THEME");
        assert_eq!(detect(&[]).background, Background::Dark);
        assert_eq!(detect(&[("COLORFGBG", "0;15")]).background, Background::Light);
        assert_eq!(detect(&[("COLORFGBG", "15;default;0")]).background, Background::Dark);
        assert_eq!(detect(&[("COLORFGBG", "0;15"), (override_name, "dark")]).background, Background::Dark);
        assert_eq!(detect(&[(override_name, "Light")]).background, Background::Light);
    }

    #[test]
    fn diff_tints_are_true_colours_only_where_the_terminal_says_so() {
        let plain = detect(&[]);
        let rich = detect(&[("COLORTERM", "truecolor")]);
        assert_eq!(plain.added_bg(), Color::Reset, "256 colours have no dark tint subtle enough: only the marks");
        assert_eq!(plain.added_mark(), Color::Indexed(114));
        assert!(matches!(rich.added_bg(), Color::Rgb(..)));
        let light = detect(&[("COLORFGBG", "0;15")]);
        assert_eq!(light.removed_bg(), Color::Indexed(224));
        assert_eq!(light.code_theme(), "InspiredGitHub");
    }
}
