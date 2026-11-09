//! Window attributes: a borderless-looking canvas with native controls.

use winit::dpi::LogicalSize;
use winit::window::WindowAttributes;

use crate::state::{Appearance, Theme};

#[cfg(target_os = "windows")]
#[path = "window/windows.rs"]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::fit_monitor;

pub fn attributes() -> WindowAttributes {
    let attributes = WindowAttributes::default()
        .with_title("Superfik")
        .with_inner_size(LogicalSize::new(1440.0, 900.0))
        .with_min_inner_size(LogicalSize::new(980.0, 640.0));
    #[cfg(target_os = "windows")]
    let attributes = {
        let icon = image::load_from_memory(include_bytes!("../../../../assets/icon.png"))
            .ok()
            .map(|image| image.into_rgba8())
            .and_then(|image| {
                let (width, height) = image.dimensions();
                winit::window::Icon::from_rgba(image.into_raw(), width, height).ok()
            });
        attributes.with_window_icon(icon)
    };
    #[cfg(target_os = "macos")]
    let attributes = {
        use winit::platform::macos::WindowAttributesExtMacOS;
        attributes.with_titlebar_transparent(true).with_title_hidden(true).with_fullsize_content_view(true)
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let attributes = {
        use winit::platform::{wayland::WindowAttributesExtWayland, x11::WindowAttributesExtX11};
        let attributes = WindowAttributesExtWayland::with_name(attributes, LINUX_APP_ID, LINUX_APP_ID);
        WindowAttributesExtX11::with_name(attributes, LINUX_APP_ID, LINUX_APP_ID)
    };
    attributes
}

#[cfg_attr(not(all(unix, not(target_os = "macos"))), allow(dead_code))]
pub const LINUX_APP_ID: &str = "superfik";

pub fn chrome(theme: Theme) -> Option<winit::window::Theme> {
    match theme {
        Theme::System => None,
        Theme::Light => Some(winit::window::Theme::Light),
        Theme::Dark => Some(winit::window::Theme::Dark),
    }
}

pub fn appearance(theme: winit::window::Theme) -> Appearance {
    match theme {
        winit::window::Theme::Light => Appearance::Light,
        winit::window::Theme::Dark => Appearance::Dark,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_linux_app_id_names_the_desktop_entry() {
        let entry = include_str!("../../../../assets/linux/superfik.desktop");
        assert!(entry.lines().any(|line| line == format!("StartupWMClass={LINUX_APP_ID}")));
        assert!(entry.lines().any(|line| line == format!("Icon={LINUX_APP_ID}")));
        assert!(entry.lines().any(|line| line == format!("Exec={LINUX_APP_ID}")));
    }

    #[test]
    fn explicit_themes_pin_the_chrome_and_system_follows_the_os() {
        assert_eq!(chrome(Theme::System), None);
        for theme in [Theme::Light, Theme::Dark] {
            let pinned = chrome(theme).map(appearance);
            assert_eq!(pinned, Some(theme.resolve(Appearance::Dark)));
            assert_eq!(pinned, Some(theme.resolve(Appearance::Light)));
        }
    }
}
