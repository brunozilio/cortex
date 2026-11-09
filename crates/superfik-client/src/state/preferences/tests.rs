use super::*;
use crate::state::Action;

#[test]
fn sound_rows_map_to_prefs_and_keep_the_pack_when_off() {
    let piano = SoundPrefs { enabled: true, pack: PackId::Piano };
    assert_eq!(SoundChoice::of(piano), SoundChoice::Pack(PackId::Piano));
    let off = SoundChoice::Off.apply(piano);
    assert_eq!(off, SoundPrefs { enabled: false, pack: PackId::Piano });
    assert_eq!(SoundChoice::of(off), SoundChoice::Off);
    assert_eq!(SoundChoice::Pack(PackId::Sine).apply(off), SoundPrefs { enabled: true, pack: PackId::Sine });
    assert_eq!(SoundChoice::Pack(PackId::Sine).label(), "Sine");
}

#[test]
fn choosing_persists_only_on_change() {
    let mut s = Session::default();
    let effects = s.apply(Action::ChooseSetting(Setting::Sound(SoundChoice::Pack(PackId::Sine))), 0.0);
    assert!(matches!(effects.as_slice(), [Effect::Persist, Effect::Sound(_)]), "{effects:?}");
    assert_eq!(s.preferences.sound.pack, PackId::Sine);
    let again = s.apply(Action::ChooseSetting(Setting::Sound(SoundChoice::Pack(PackId::Sine))), 0.0);
    assert!(again.is_empty(), "{again:?}");
    assert!(s.apply(Action::ChooseSetting(Setting::Theme(Theme::System)), 0.0).is_empty());
    assert!(s.preferences.is_current(Setting::Language(Language::English)));
}

#[test]
fn restored_preferences_replace_defaults() {
    let mut s = Session::default();
    let prefs = Preferences { sound: SoundPrefs { enabled: false, pack: PackId::Sine }, ..Preferences::default() };
    assert!(s.apply(Action::RestorePreferences(prefs), 0.0).is_empty());
    assert_eq!(s.preferences, prefs);
}

#[test]
fn preferences_serialise_with_stable_names() {
    let json = serde_json::to_string(&Preferences::default()).expect("json");
    assert_eq!(
        json,
        r#"{"sound":{"enabled":true,"pack":"piano"},"haptics":true,"language":"en","theme":"system","fill_on_load":true,"voice":{"recognizer":{"family":"nemotron35","chunk":"560ms"},"language":"pt-BR","voice":"pm_santa"},"voice_enabled":true,"wake":{"enabled":false,"word":"Piratinha"},"voice_address":"Capitão","output_style":"default","suggestions":true,"safety_check":true,"heads_up":true,"lock":false}"#
    );
    let partial: Preferences = serde_json::from_str(r#"{"language":"en"}"#).expect("parses");
    assert_eq!(partial, Preferences::default());
    for theme in Theme::ALL {
        let json = serde_json::to_string(&Preferences { theme, ..Preferences::default() }).expect("json");
        let back: Preferences = serde_json::from_str(&json).expect("parses");
        assert_eq!(back.theme, theme);
    }
    let saved: Preferences = serde_json::from_str(r#"{"theme":"dark"}"#).expect("parses");
    assert_eq!(saved.theme, Theme::Dark);
}

#[test]
fn haptics_default_on_and_toggle_with_persist() {
    let old: Preferences = serde_json::from_str(r#"{"theme":"dark"}"#).expect("parses");
    assert!(old.haptics, "settings saved before the field keep haptics on");
    assert!(old.fill_on_load, "vault logins fill on load unless turned off");
    let mut s = Session::default();
    assert!(s.preferences.is_current(Setting::Haptics(true)));
    let effects = s.apply(Action::ChooseSetting(Setting::Haptics(false)), 0.0);
    assert!(matches!(effects.first(), Some(Effect::Persist)), "{effects:?}");
    assert!(!s.preferences.haptics);
    assert!(s.apply(Action::ChooseSetting(Setting::Haptics(false)), 0.0).is_empty());
}

#[test]
fn themes_resolve_against_the_system_appearance() {
    for system in [Appearance::Light, Appearance::Dark] {
        assert_eq!(Theme::System.resolve(system), system);
        assert_eq!(Theme::Light.resolve(system), Appearance::Light);
        assert_eq!(Theme::Dark.resolve(system), Appearance::Dark);
    }
}

#[test]
fn system_theme_follows_the_os_without_persisting() {
    let mut s = Session::default();
    assert_eq!(s.appearance(), Appearance::Dark);
    assert!(s.apply(Action::SystemAppearance(Appearance::Light), 0.0).is_empty());
    assert_eq!(s.appearance(), Appearance::Light);
    let effects = s.apply(Action::ChooseSetting(Setting::Theme(Theme::Dark)), 0.0);
    assert!(matches!(effects.first(), Some(Effect::Persist)), "{effects:?}");
    assert_eq!(s.appearance(), Appearance::Dark);
    s.apply(Action::SystemAppearance(Appearance::Dark), 0.0);
    s.apply(Action::ChooseSetting(Setting::Theme(Theme::Light)), 0.0);
    assert_eq!(s.appearance(), Appearance::Light);
}
