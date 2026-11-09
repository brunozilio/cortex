//! The Settings screen: whether it covers the window and which pane it
//! shows.

use super::{Effect, Platform, Session};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SettingsPane {
    #[default]
    General,
    Sound,
    Ai,
    Voice,
    Browser,
    Integrations,
    Vaults,
    Server,
}

impl SettingsPane {
    pub const ALL: [SettingsPane; 8] = [
        SettingsPane::General,
        SettingsPane::Sound,
        SettingsPane::Ai,
        SettingsPane::Voice,
        SettingsPane::Browser,
        SettingsPane::Integrations,
        SettingsPane::Vaults,
        SettingsPane::Server,
    ];

    pub fn offered(platform: Platform) -> impl Iterator<Item = SettingsPane> {
        let sound = platform.sound || platform.haptics;
        Self::ALL.into_iter().filter(move |pane| match pane {
            SettingsPane::Sound => sound,
            SettingsPane::Voice => platform.voice,
            _ => true,
        })
    }

    pub const fn lists_plugins(self) -> bool {
        matches!(self, SettingsPane::Ai | SettingsPane::Integrations | SettingsPane::Vaults)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SettingsScreen {
    pub open: bool,
    pub pane: SettingsPane,
}

impl Session {
    pub(super) fn open_settings(&mut self) -> Vec<Effect> {
        self.settings.open = true;
        self.popover = None;
        let mut effects = self.refresh_plugins();
        if self.settings.pane == SettingsPane::Server {
            effects.extend(self.refresh_members());
            effects.extend(self.server_pane_shown());
        }
        effects.push(Session::list_channels());
        if !self.browser_profiles.loaded {
            effects.push(Effect::LoadBrowserProfiles);
        }
        effects
    }

    pub(super) fn close_settings(&mut self) -> Vec<Effect> {
        self.settings.open = false;
        self.plugins.panel = Default::default();
        self.browser_profiles.editor = None;
        self.browser_profiles.error = None;
        self.server_link.focus = None;
        self.keep_voice_field()
    }

    pub(super) fn show_settings_pane(&mut self, pane: SettingsPane) -> Vec<Effect> {
        if !SettingsPane::offered(self.platform).any(|offered| offered == pane) {
            return Vec::new();
        }
        if pane != SettingsPane::Browser {
            self.browser_profiles.editor = None;
        }
        self.settings.pane = pane;
        self.plugins.panel = Default::default();
        self.server_link.focus = None;
        let mut effects = self.keep_voice_field();
        effects.extend(if pane == SettingsPane::Server {
            let mut effects = self.refresh_members();
            effects.extend(self.server_pane_shown());
            effects
        } else {
            self.refresh_plugins()
        });
        effects
    }

    fn refresh_plugins(&self) -> Vec<Effect> {
        if self.plugins_pane_open() {
            let mut effects = vec![Effect::Command(Box::new(superfik_protocol::ClientCommand::ListPlugins))];
            if self.settings.pane == SettingsPane::Ai {
                effects.push(Effect::Command(Box::new(superfik_protocol::ClientCommand::ListModels)));
            }
            effects
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Action, PopoverKind};

    #[test]
    fn a_browser_tab_has_no_sound_pane() {
        assert!(SettingsPane::offered(Platform::DESKTOP).eq(SettingsPane::ALL));
        assert!(!SettingsPane::offered(Platform::WEB).any(|pane| pane == SettingsPane::Sound));
        let mut web = Session { platform: Platform::WEB, ..Session::default() };
        web.apply(Action::OpenSettings, 0.0);
        web.apply(Action::ShowSettingsPane(SettingsPane::Sound), 0.0);
        assert_eq!(web.settings.pane, SettingsPane::General);
    }

    #[test]
    fn opens_closes_and_reopens_on_the_last_pane() {
        let mut s = Session { popover: Some(PopoverKind::Model), ..Session::default() };
        assert_eq!(s.settings, SettingsScreen { open: false, pane: SettingsPane::General });
        s.apply(Action::OpenSettings, 0.0);
        assert!(s.settings.open);
        assert_eq!(s.popover, None, "a popover gives way to the screen");
        s.apply(Action::ShowSettingsPane(SettingsPane::Sound), 0.0);
        s.apply(Action::CloseSettings, 0.0);
        assert!(!s.settings.open);
        s.apply(Action::OpenSettings, 0.0);
        assert_eq!(s.settings, SettingsScreen { open: true, pane: SettingsPane::Sound });
    }

    #[test]
    fn choosing_a_setting_keeps_the_screen_open() {
        let mut s = Session::default();
        s.apply(Action::OpenSettings, 0.0);
        let effects = s.apply(Action::ChooseSetting(crate::state::Setting::Haptics(false)), 0.0);
        assert!(matches!(effects.first(), Some(Effect::Persist)), "{effects:?}");
        assert!(s.settings.open);
    }
}
