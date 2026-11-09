//! The application menu.
//!
//! [`AppMenu`] is the app-facing seam: [`system`] adds "Settings…" (⌘,) to
//! the macOS app menu, where every Mac app keeps it, and calls back when it
//! is chosen. [`MacMenu`] does it with AppKit; [`Plain`] stands in where
//! there is no such menu; [`Recorder`] keeps what it was told, for tests.

#[cfg(target_os = "macos")]
mod mac;

#[cfg(target_os = "macos")]
pub use mac::MacMenu;

pub trait AppMenu {
    fn set_settings_title(&mut self, title: &str);
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Plain;

impl AppMenu for Plain {
    fn set_settings_title(&mut self, _title: &str) {}
}

#[derive(Debug, Default)]
pub struct Recorder {
    pub titles: Vec<String>,
}

impl AppMenu for Recorder {
    fn set_settings_title(&mut self, title: &str) {
        self.titles.push(title.to_owned());
    }
}

pub fn system(title: &str, on_settings: Box<dyn Fn()>) -> Box<dyn AppMenu> {
    #[cfg(target_os = "macos")]
    if let Some(menu) = MacMenu::install(title, on_settings) {
        return Box::new(menu);
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (title, on_settings);
    Box::new(Plain)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recorder_keeps_titles_in_order() {
        let mut menu = Recorder::default();
        menu.set_settings_title("Settings…");
        menu.set_settings_title("Ajustes…");
        assert_eq!(menu.titles, vec!["Settings…", "Ajustes…"]);
    }
}
