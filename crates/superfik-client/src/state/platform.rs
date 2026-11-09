//! What the client running this session can do (ADR 0010). The desktop
//! can do everything; a browser tab has no speakers we may start on our
//! own, no trackpad actuator, no Keychain, no other browser's cookies and
//! no native dialogs, and the browser keeps its own shortcuts. The reducer
//! never asks for what the platform lacks, and the views hide it, so
//! nothing below the shell needs a `cfg`.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Primary {
    Command,
    Control,
}

impl Primary {
    pub fn label(self) -> &'static str {
        match self {
            Self::Command => "⌘",
            Self::Control => "Ctrl+",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemUnlock {
    TouchId,
    WindowsHello,
}

impl SystemUnlock {
    pub fn name(self) -> &'static str {
        match self {
            Self::TouchId => "Touch ID",
            Self::WindowsHello => "Windows Hello",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Platform {
    pub custom_titlebar: bool,
    pub primary: Primary,
    pub sound: bool,
    pub haptics: bool,
    pub get_started: bool,
    pub system_unlock: Option<SystemUnlock>,
    pub import_logins: bool,
    pub native_dialogs: bool,
    pub page_input: bool,
    pub pages: bool,
    pub browser_shortcuts: bool,
    pub voice: bool,
}

impl Platform {
    pub const DESKTOP: Self = Self {
        custom_titlebar: true,
        primary: Primary::Command,
        voice: true,
        sound: true,
        haptics: true,
        get_started: true,
        system_unlock: Some(SystemUnlock::TouchId),
        import_logins: true,
        native_dialogs: true,
        page_input: true,
        pages: true,
        browser_shortcuts: true,
    };

    pub const WINDOWS: Self = Self {
        custom_titlebar: false,
        primary: Primary::Control,
        haptics: false,
        system_unlock: Some(SystemUnlock::WindowsHello),
        import_logins: false,
        page_input: false,
        pages: false,
        ..Self::DESKTOP
    };

    pub const LINUX: Self =
        Self { page_input: true, pages: true, import_logins: true, system_unlock: None, ..Self::WINDOWS };

    pub const WEB: Self = Self {
        custom_titlebar: false,
        primary: Primary::Command,
        sound: false,
        haptics: false,
        get_started: false,
        system_unlock: None,
        import_logins: false,
        native_dialogs: false,
        page_input: true,
        pages: true,
        browser_shortcuts: false,
        voice: false,
    };

    pub fn titlebar_height(self) -> f32 {
        if self.custom_titlebar { superfik_ui::theme::metrics::TITLEBAR_HEIGHT } else { 0.0 }
    }
}

impl Default for Platform {
    fn default() -> Self {
        Self::DESKTOP
    }
}

#[cfg(test)]
mod tests {
    use superfik_browser::import::BrowserKind;

    use super::*;
    use crate::state::{Action, Effect, PopoverKind, Session};

    fn on(platform: Platform) -> Session {
        Session { platform, ..Session::default() }
    }

    #[test]
    fn changing_workspaces_preserves_the_platform_and_its_capabilities() {
        for platform in [Platform::DESKTOP, Platform::WINDOWS, Platform::LINUX, Platform::WEB] {
            let mut session = on(platform);
            for path in ["/repo", "/other"] {
                session.apply(Action::OpenWorkspace(path.into()), 0.0);
                assert_eq!(session.platform, platform, "opening {path} changed the platform");
            }
            session.apply(Action::RemoveWorkspace, 0.0);
            assert_eq!(session.platform, platform, "removing the workspace changed the platform");
        }
    }

    #[test]
    fn the_desktop_can_do_everything() {
        assert_eq!(Session::default().platform, Platform::DESKTOP);
    }

    #[test]
    fn native_decorations_and_system_unlocks_follow_the_platform() {
        assert_eq!(Platform::WINDOWS.titlebar_height(), 0.0);
        assert_eq!(Platform::LINUX.titlebar_height(), 0.0);
        assert_eq!(Platform::WEB.titlebar_height(), 0.0);
        assert_eq!(Platform::DESKTOP.titlebar_height(), 28.0);
        assert_eq!(Platform::WINDOWS.system_unlock, Some(SystemUnlock::WindowsHello));
        assert_eq!(Platform::DESKTOP.system_unlock, Some(SystemUnlock::TouchId));
    }

    #[test]
    fn the_web_makes_no_sound() {
        let mut desktop = on(Platform::DESKTOP);
        let mut web = on(Platform::WEB);
        let open = || Action::OpenPopover(PopoverKind::Model);
        assert!(desktop.apply(open(), 0.0).iter().any(|e| matches!(e, Effect::Sound(_))));
        assert!(!web.apply(open(), 0.0).iter().any(|e| matches!(e, Effect::Sound(_))));
        assert_eq!(web.popover, Some(PopoverKind::Model), "the action itself still happens");
    }

    #[test]
    fn the_web_never_shows_get_started() {
        let mut web = on(Platform::WEB);
        web.apply(Action::ShowGetStarted, 0.0);
        assert!(!web.get_started);
    }

    #[test]
    fn windows_reads_no_other_browsers_logins_yet() {
        let mut windows = on(Platform::WINDOWS);
        windows.logins.sources = vec![BrowserKind::Chrome];
        let effects = windows.apply(Action::ImportLogins(BrowserKind::Chrome), 0.0);
        assert!(!effects.iter().any(|e| matches!(e, Effect::ImportLogins(_))), "{effects:?}");
        assert_eq!(Platform::WINDOWS.primary, Primary::Control);
        let mut sounding = on(Platform::WINDOWS);
        let open = Action::OpenPopover(PopoverKind::Model);
        assert!(sounding.apply(open, 0.0).iter().any(|e| matches!(e, Effect::Sound(_))), "it still has speakers");
    }

    #[test]
    fn linux_takes_ctrl_and_has_no_trackpad_or_system_unlock() {
        let linux = Platform::LINUX;
        assert_eq!(linux.primary, Primary::Control);
        assert!(!linux.haptics && linux.system_unlock.is_none());
        assert!(linux.sound && linux.native_dialogs && linux.browser_shortcuts);
    }

    #[test]
    fn linux_opens_web_pages_and_windows_not_yet() {
        let mut linux = on(Platform::LINUX);
        let effects = linux.apply(Action::OpenBrowser, 0.0);
        assert!(linux.browser().is_some(), "a card opens");
        assert!(effects.iter().any(|e| matches!(e, Effect::Browser(_))), "{effects:?}");
        assert_eq!((Platform::LINUX.page_input, Platform::WINDOWS.pages), (true, false));
    }

    #[test]
    fn linux_brings_logins_over_from_other_browsers() {
        let mut linux = on(Platform::LINUX);
        linux.logins.sources = vec![BrowserKind::Chrome];
        let effects = linux.apply(Action::ImportLogins(BrowserKind::Chrome), 0.0);
        assert!(effects.iter().any(|e| matches!(e, Effect::ImportLogins(BrowserKind::Chrome))), "{effects:?}");
    }

    #[test]
    fn the_web_never_imports_logins() {
        let mut web = on(Platform::WEB);
        web.logins.sources = vec![BrowserKind::Chrome];
        let effects = web.apply(Action::ImportLogins(BrowserKind::Chrome), 0.0);
        assert!(!effects.iter().any(|e| matches!(e, Effect::ImportLogins(_))), "{effects:?}");
        assert!(web.logins.importing.is_none());
    }
}
