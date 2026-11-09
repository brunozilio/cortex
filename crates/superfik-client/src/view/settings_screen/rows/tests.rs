use superfik_sound::SoundPrefs;

use super::*;

fn lines(groups: &[Group]) -> Vec<String> {
    let mut out = Vec::new();
    for group in groups {
        if let Some(heading) = &group.heading {
            out.push(format!("# {heading}"));
        }
        for line in &group.lines {
            let control = match &line.control {
                Control::Switch { on, .. } => if *on { "(on)" } else { "(off)" }.to_owned(),
                Control::Choice { options, selected, enabled } => {
                    let names = options
                        .iter()
                        .enumerate()
                        .map(|(i, (name, _))| if i == *selected { format!("[{name}]") } else { name.clone() });
                    let names = names.collect::<Vec<_>>().join(" ");
                    if *enabled { names } else { format!("{{{names}}}") }
                }
                Control::None => String::new(),
                Control::Field { edit, .. } => format!("[{}]", edit.text()),
                Control::Buttons(buttons) => {
                    buttons.iter().map(|(label, ..)| format!("<{label}>")).collect::<Vec<_>>().join(" ")
                }
                Control::Action { labels: [label, ..], run, .. } => {
                    if run.is_some() {
                        format!("<{label}>")
                    } else {
                        format!("{{{label}}}")
                    }
                }
            };
            out.push(format!("{}: {control}", line.title));
        }
    }
    out
}

#[test]
fn general_offers_the_languages_and_themes_with_the_current_ones_chosen() {
    let prefs = Preferences { theme: Theme::Dark, ..Preferences::default() };
    let groups = groups(SettingsPane::General, prefs, &Strings::new(Language::English));
    assert_eq!(
        lines(&groups),
        vec![
            "Language: [English] Português",
            "Theme: System Light [Dark]",
            "Response style: [Default] Concise Proactive Explanatory Learning",
            "Suggest the next prompt: (on)",
            "Check autonomous actions: (on)",
            "You should know: (on)",
        ]
    );
    assert_eq!(groups[0].lines[3].control.setting(0), Some(Setting::Suggestions(false)));
    let style = &groups[0].lines[2].control;
    assert_eq!(style.setting(1), Some(Setting::OutputStyle(OutputStyle::Concise)));
    let language = &groups[0].lines[0].control;
    assert_eq!(language.setting(1), Some(Setting::Language(Language::Portuguese)));
    assert_eq!(language.part_of(Setting::Language(Language::Portuguese)), Some(1));
    assert!(groups[0].lines.iter().all(|line| line.detail.is_some()), "every row explains itself");
}

#[test]
fn sound_has_the_switch_the_packs_and_haptics() {
    let groups = groups(SettingsPane::Sound, Preferences::default(), &Strings::new(Language::English));
    assert_eq!(
        lines(&groups),
        vec![
            "# Sounds",
            "Play interface sounds: (on)",
            "Sound pack: [Piano] Sine Internal",
            "# Haptic feedback",
            "Pulse the trackpad: (on)",
        ]
    );
    let switch = &groups[0].lines[0].control;
    assert_eq!(switch.setting(0), Some(Setting::Sound(SoundChoice::Off)), "the switch turns sound off");
    assert_eq!(switch.setting(1), None);
    assert_eq!(groups[0].lines[1].control.setting(2), Some(Setting::Sound(SoundChoice::Pack(PackId::Internal))));
    assert_eq!(groups[1].lines[0].control.setting(0), Some(Setting::Haptics(false)));
}

#[test]
fn with_sound_off_the_packs_wait_and_the_switch_brings_the_pack_back() {
    let prefs = Preferences { sound: SoundPrefs { enabled: false, pack: PackId::Sine }, ..Preferences::default() };
    let groups = groups(SettingsPane::Sound, prefs, &Strings::new(Language::English));
    assert_eq!(lines(&groups)[1..3], ["Play interface sounds: (off)", "Sound pack: {Piano [Sine] Internal}"]);
    assert_eq!(groups[0].lines[0].control.setting(0), Some(Setting::Sound(SoundChoice::Pack(PackId::Sine))));
}

#[test]
fn rows_read_in_portuguese() {
    let prefs = Preferences { language: Language::Portuguese, ..Preferences::default() };
    let strings = Strings::new(prefs.language);
    assert_eq!(lines(&groups(SettingsPane::General, prefs, &strings))[1], "Tema: [Sistema] Claro Escuro");
    assert_eq!(lines(&groups(SettingsPane::Sound, prefs, &strings))[1], "Tocar sons da interface: (on)");
}
