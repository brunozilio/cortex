//! User preferences shown on the Settings screen. Pure; persisted by the
//! shell through `Effect::Persist`.

use serde::{Deserialize, Serialize};
use superfik_sound::{PackId, SoundPrefs};
use superfik_voice::{Recognizer, VoicePrefs, WakePrefs};

pub use superfik_i18n::Language;

use super::{Effect, Session};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

impl Theme {
    pub const ALL: [Theme; 3] = [Theme::System, Theme::Light, Theme::Dark];

    pub fn label(self) -> &'static str {
        match self {
            Theme::System => "System",
            Theme::Light => "Light",
            Theme::Dark => "Dark",
        }
    }

    pub fn resolve(self, system: Appearance) -> Appearance {
        match self {
            Theme::System => system,
            Theme::Light => Appearance::Light,
            Theme::Dark => Appearance::Dark,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Appearance {
    Light,
    #[default]
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SoundChoice {
    Off,
    Pack(PackId),
}

impl SoundChoice {
    pub fn label(self) -> &'static str {
        match self {
            SoundChoice::Off => "Off",
            SoundChoice::Pack(PackId::Piano) => "Piano",
            SoundChoice::Pack(PackId::Sine) => "Sine",
            SoundChoice::Pack(PackId::Internal) => "Internal",
        }
    }

    pub fn of(prefs: SoundPrefs) -> Self {
        if prefs.enabled { SoundChoice::Pack(prefs.pack) } else { SoundChoice::Off }
    }

    pub fn apply(self, prefs: SoundPrefs) -> SoundPrefs {
        match self {
            SoundChoice::Off => SoundPrefs { enabled: false, ..prefs },
            SoundChoice::Pack(pack) => SoundPrefs { enabled: true, pack },
        }
    }
}

pub use super::setting::Setting;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub sound: SoundPrefs,
    pub haptics: bool,
    pub language: Language,
    pub theme: Theme,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logins: Option<superfik_browser::import::BrowserKind>,
    pub fill_on_load: bool,
    pub voice: superfik_voice::VoicePrefs,
    pub voice_enabled: bool,
    pub wake: superfik_voice::WakePrefs,
    pub voice_address: superfik_voice::Address,
    pub output_style: superfik_core::OutputStyle,
    pub suggestions: bool,
    pub safety_check: bool,
    pub heads_up: bool,
    pub lock: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            sound: SoundPrefs::default(),
            haptics: true,
            language: Language::default(),
            theme: Theme::default(),
            logins: None,
            fill_on_load: true,
            voice: superfik_voice::VoicePrefs::default(),
            voice_enabled: true,
            wake: superfik_voice::WakePrefs::default(),
            voice_address: superfik_voice::Address::default(),
            output_style: superfik_core::OutputStyle::Default,
            suggestions: true,
            safety_check: true,
            heads_up: true,
            lock: false,
        }
    }
}

fn heard(language: Language) -> superfik_voice::Language {
    match language {
        Language::English => superfik_voice::Language::English,
        Language::Portuguese => superfik_voice::Language::PortugueseBrazil,
    }
}

impl Preferences {
    pub fn with(self, setting: Setting) -> Self {
        match setting {
            Setting::Sound(choice) => Self { sound: choice.apply(self.sound), ..self },
            Setting::Haptics(haptics) => Self { haptics, ..self },
            Setting::Language(language) => Self { language, ..self },
            Setting::Theme(theme) => Self { theme, ..self },
            Setting::FillOnLoad(fill_on_load) => Self { fill_on_load, ..self },
            Setting::Lock(lock) => Self { lock, ..self },
            Setting::VoiceEnabled(voice_enabled) => Self { voice_enabled, ..self },
            Setting::VoiceChunk(chunk) => {
                let recognizer = Recognizer::Nemotron35(chunk);
                Self { voice: VoicePrefs { recognizer, ..self.voice }, ..self }
            }
            Setting::VoiceSpeaker(voice) => Self { voice: VoicePrefs { voice, ..self.voice }, ..self },
            Setting::WakeEnabled(enabled) => Self { wake: WakePrefs { enabled, ..self.wake }, ..self },
            Setting::WakeWord(word) => Self { wake: WakePrefs { word, ..self.wake }, ..self },
            Setting::VoiceAddress(voice_address) => Self { voice_address, ..self },
            Setting::OutputStyle(output_style) => Self { output_style, ..self },
            Setting::Suggestions(suggestions) => Self { suggestions, ..self },
            Setting::SafetyCheck(safety_check) => Self { safety_check, ..self },
            Setting::HeadsUp(heads_up) => Self { heads_up, ..self },
            Setting::Policy(_) | Setting::Team(_) => self,
        }
    }

    pub fn voice_prefs(self) -> VoicePrefs {
        VoicePrefs {
            recognizer: Recognizer::Nemotron35(self.voice.recognizer.chunk()),
            language: heard(self.language),
            voice: self.voice.voice,
        }
    }

    pub fn is_current(self, setting: Setting) -> bool {
        match setting {
            Setting::Sound(choice) => SoundChoice::of(self.sound) == choice,
            Setting::Haptics(haptics) => self.haptics == haptics,
            Setting::Language(language) => self.language == language,
            Setting::Theme(theme) => self.theme == theme,
            Setting::FillOnLoad(fill_on_load) => self.fill_on_load == fill_on_load,
            Setting::Lock(lock) => self.lock == lock,
            Setting::VoiceEnabled(on) => self.voice_enabled == on,
            Setting::VoiceChunk(chunk) => self.voice.recognizer.chunk() == chunk,
            Setting::VoiceSpeaker(voice) => self.voice.voice == voice,
            Setting::WakeEnabled(enabled) => self.wake.enabled == enabled,
            Setting::WakeWord(word) => self.wake.word == word,
            Setting::VoiceAddress(address) => self.voice_address == address,
            Setting::OutputStyle(style) => self.output_style == style,
            Setting::Suggestions(on) => self.suggestions == on,
            Setting::SafetyCheck(on) => self.safety_check == on,
            Setting::HeadsUp(on) => self.heads_up == on,
            Setting::Policy(_) | Setting::Team(_) => false,
        }
    }
}

impl Session {
    pub fn strings(&self) -> superfik_i18n::Strings {
        superfik_i18n::Strings::new(self.preferences.language)
    }

    pub(super) fn choose_setting(&mut self, setting: Setting) -> Vec<Effect> {
        match setting {
            Setting::Policy(choice) => return self.choose_policy(choice),
            Setting::Team(choice) => return self.choose_team(choice),
            Setting::Lock(true) if !self.offers_lock() => return Vec::new(),
            _ => {}
        }
        let next = self.preferences.with(setting);
        if next == self.preferences {
            return Vec::new();
        }
        self.preferences = next;
        let mut effects = vec![Effect::Persist];
        effects.extend(self.voice_preferences_changed(setting));
        effects
    }

    pub fn appearance(&self) -> Appearance {
        self.preferences.theme.resolve(self.system_appearance)
    }
}

#[cfg(test)]
mod tests;
