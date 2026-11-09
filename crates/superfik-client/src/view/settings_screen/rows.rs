//! What each pane of the Settings screen lists, as groups of rows: pure,
//! from the preferences and the interface language.

use superfik_browser::import::BrowserKind;
use superfik_i18n::{Strings, Text};
use superfik_sound::PackId;
use superfik_ui::components::Progress;

pub use super::field::Field;
use crate::state::{Language, Preferences, Setting, SettingsPane, SoundChoice, Theme};
use superfik_core::OutputStyle;

#[derive(Debug, Clone, PartialEq)]
pub enum Control {
    Switch { on: bool, flip: Setting },
    Choice { options: Vec<(String, Setting)>, selected: usize, enabled: bool },
    Action { labels: [String; 3], icon: superfik_render::Icon, run: Option<Run>, progress: Progress },
    Buttons(Vec<(String, superfik_render::Icon, Run)>),
    Field { edit: superfik_ui::TextEdit, focused: bool, field: Field },
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Run {
    ImportLogins(BrowserKind),
    ClearCookies,
    Server(ServerRun),
    AnswerDelegated(usize, bool),
    RemoveFromLibrary(usize),
    Team(crate::state::TeamChoice),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerRun {
    Join,
    AllowInsecure,
    ToggleFirstAccess,
    Leave,
    AddUser,
    NewUserRole(superfik_core::access::OrgRole),
    NewCode(superfik_core::UserId),
    ToggleAdmin(superfik_core::UserId),
    Remove(superfik_core::UserId),
    CopyCode,
    DismissCode,
    SaveSettings,
}

impl From<ServerRun> for crate::state::ServerAction {
    fn from(run: ServerRun) -> Self {
        use crate::state::ServerAction as A;
        match run {
            ServerRun::Join => A::Join,
            ServerRun::AllowInsecure => A::AllowInsecure,
            ServerRun::ToggleFirstAccess => A::ToggleFirstAccess,
            ServerRun::Leave => A::Leave,
            ServerRun::AddUser => A::AddUser,
            ServerRun::NewUserRole(role) => A::NewUserRole(role),
            ServerRun::NewCode(user) => A::NewCode(user),
            ServerRun::ToggleAdmin(user) => A::ToggleAdmin(user),
            ServerRun::Remove(user) => A::Remove(user),
            ServerRun::CopyCode => A::CopyCode,
            ServerRun::DismissCode => A::DismissCode,
            ServerRun::SaveSettings => A::SaveSettings,
        }
    }
}

impl Control {
    pub fn setting(&self, index: usize) -> Option<Setting> {
        match self {
            Control::Switch { flip, .. } => (index == 0).then_some(*flip),
            Control::Choice { options, .. } => options.get(index).map(|(_, setting)| *setting),
            Control::Action { .. } | Control::Buttons(_) | Control::Field { .. } | Control::None => None,
        }
    }

    pub fn part_of(&self, setting: Setting) -> Option<usize> {
        match self {
            Control::Switch { flip, .. } => (*flip == setting).then_some(0),
            Control::Choice { options, .. } => options.iter().position(|(_, s)| *s == setting),
            Control::Action { .. } | Control::Buttons(_) | Control::Field { .. } | Control::None => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub browser: Option<BrowserKind>,
    pub title: String,
    pub detail: Option<String>,
    pub control: Control,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    pub heading: Option<String>,
    pub lines: Vec<Line>,
}

pub fn pane_name(pane: SettingsPane) -> Text<'static> {
    match pane {
        SettingsPane::General => Text::SettingsGeneral,
        SettingsPane::Sound => Text::SettingsSound,
        SettingsPane::Ai => Text::SettingsAi,
        SettingsPane::Voice => Text::SettingsVoice,
        SettingsPane::Browser => Text::SettingsBrowser,
        SettingsPane::Integrations => Text::SettingsIntegrations,
        SettingsPane::Vaults => Text::SettingsVaults,
        SettingsPane::Server => Text::SettingsServer,
    }
}

pub fn pane_caption(pane: SettingsPane) -> Text<'static> {
    match pane {
        SettingsPane::General => Text::SettingsGeneralCaption,
        SettingsPane::Sound => Text::SettingsSoundCaption,
        SettingsPane::Ai => Text::SettingsAiCaption,
        SettingsPane::Voice => Text::SettingsVoiceCaption,
        SettingsPane::Browser => Text::SettingsBrowserCaption,
        SettingsPane::Integrations => Text::SettingsIntegrationsCaption,
        SettingsPane::Vaults => Text::SettingsVaultsCaption,
        SettingsPane::Server => Text::SettingsServerCaption,
    }
}

pub fn lock(preferences: Preferences, strings: &Strings, method: &str) -> Group {
    let on = preferences.lock;
    Group {
        heading: None,
        lines: vec![Line {
            browser: None,
            title: strings.t(Text::SettingsLock(method)),
            detail: Some(strings.t(Text::SettingsLockCaption(method))),
            control: Control::Switch { on, flip: Setting::Lock(!on) },
        }],
    }
}

pub fn groups(pane: SettingsPane, preferences: Preferences, strings: &Strings) -> Vec<Group> {
    let t = |text: Text<'_>| strings.t(text);
    let line = |title: Text<'_>, detail: Text<'_>, control: Control| Line {
        browser: None,
        title: t(title),
        detail: Some(t(detail)),
        control,
    };
    let choice = |options: Vec<(String, Setting)>, enabled: bool| {
        let selected = options.iter().position(|(_, s)| preferences.is_current(*s)).unwrap_or(0);
        Control::Choice { options, selected, enabled }
    };
    match pane {
        SettingsPane::General => {
            let languages = Language::ALL.map(|l| (l.native_name().to_owned(), Setting::Language(l)));
            let theme = |theme: Theme| match theme {
                Theme::System => Text::ThemeSystem,
                Theme::Light => Text::ThemeLight,
                Theme::Dark => Text::ThemeDark,
            };
            let themes = Theme::ALL.map(|th| (t(theme(th)), Setting::Theme(th)));
            let styles = OutputStyle::ALL.map(|style| (t(Text::OutputStyleName(style)), Setting::OutputStyle(style)));
            vec![Group {
                heading: None,
                lines: vec![
                    line(Text::SettingsLanguage, Text::SettingsLanguageCaption, choice(languages.to_vec(), true)),
                    line(Text::SettingsTheme, Text::SettingsThemeCaption, choice(themes.to_vec(), true)),
                    line(Text::SettingsOutputStyle, Text::SettingsOutputStyleCaption, choice(styles.to_vec(), true)),
                    line(
                        Text::SettingsSuggestions,
                        Text::SettingsSuggestionsCaption,
                        Control::Switch {
                            on: preferences.suggestions,
                            flip: Setting::Suggestions(!preferences.suggestions),
                        },
                    ),
                    line(
                        Text::SettingsSafetyCheck,
                        Text::SettingsSafetyCheckCaption,
                        Control::Switch {
                            on: preferences.safety_check,
                            flip: Setting::SafetyCheck(!preferences.safety_check),
                        },
                    ),
                    line(
                        Text::SettingsHeadsUp,
                        Text::SettingsHeadsUpCaption,
                        Control::Switch { on: preferences.heads_up, flip: Setting::HeadsUp(!preferences.heads_up) },
                    ),
                ],
            }]
        }
        SettingsPane::Sound => {
            let sound = preferences.sound;
            let flip = if sound.enabled { SoundChoice::Off } else { SoundChoice::Pack(sound.pack) };
            let packs = PackId::ALL
                .into_iter()
                .map(|pack| (SoundChoice::Pack(pack).label().to_owned(), Setting::Sound(SoundChoice::Pack(pack))))
                .collect::<Vec<_>>();
            let selected = PackId::ALL.iter().position(|&pack| pack == sound.pack).unwrap_or(0);
            let haptics = preferences.haptics;
            vec![
                Group {
                    heading: Some(t(Text::SettingsSounds)),
                    lines: vec![
                        line(
                            Text::SettingsPlaySounds,
                            Text::SettingsPlaySoundsCaption,
                            Control::Switch { on: sound.enabled, flip: Setting::Sound(flip) },
                        ),
                        line(
                            Text::SettingsSoundPack,
                            Text::SettingsSoundPackCaption,
                            Control::Choice { options: packs, selected, enabled: sound.enabled },
                        ),
                    ],
                },
                Group {
                    heading: Some(t(Text::SettingsHaptics)),
                    lines: vec![line(
                        Text::SettingsPulseTrackpad,
                        Text::SettingsHapticsCaption,
                        Control::Switch { on: haptics, flip: Setting::Haptics(!haptics) },
                    )],
                },
            ]
        }
        SettingsPane::Browser => {
            let on = preferences.fill_on_load;
            vec![Group {
                heading: None,
                lines: vec![line(
                    Text::SettingsFillOnLoad,
                    Text::SettingsFillOnLoadCaption,
                    Control::Switch { on, flip: Setting::FillOnLoad(!on) },
                )],
            }]
        }
        SettingsPane::Voice => super::voice::groups(preferences, None, strings),
        SettingsPane::Ai | SettingsPane::Integrations | SettingsPane::Vaults | SettingsPane::Server => Vec::new(),
    }
}

#[cfg(test)]
mod tests;
