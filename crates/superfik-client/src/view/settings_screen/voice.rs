//! The Voice pane's rows: the voice agent on or off, the wake word (its
//! switch and the word), what the agent calls the user, how long it listens before answering, and which
//! voice answers. It hears the interface's language. Pure.

use superfik_i18n::{Strings, Text};
use superfik_voice::{Chunk, Voice};

use superfik_ui::TextEdit;

use super::rows::{Control, Field, Group, Line};
use crate::state::{Preferences, Setting, VoiceField};

pub fn groups(preferences: Preferences, draft: Option<&(VoiceField, TextEdit)>, strings: &Strings) -> Vec<Group> {
    let on = preferences.voice_enabled;
    let line = |title, detail: Option<Text>, control| Line {
        browser: None,
        title: strings.t(title),
        detail: detail.map(|d| strings.t(d)),
        control,
    };
    let choice = |options: Vec<(String, Setting)>| {
        let selected = options.iter().position(|(_, s)| preferences.is_current(*s)).unwrap_or(0);
        Control::Choice { options, selected, enabled: on }
    };
    let chunks = Chunk::ALL.map(|c| (format!("{} ms", c.tag().trim_end_matches("ms")), Setting::VoiceChunk(c)));
    let voices = Voice::ALL.map(|v| (speaker_name(v).to_owned(), Setting::VoiceSpeaker(v)));
    let mut groups = vec![Group {
        heading: None,
        lines: vec![line(
            Text::SettingsVoiceEnabled,
            Some(Text::SettingsVoiceEnabledCaption),
            Control::Switch { on, flip: Setting::VoiceEnabled(!on) },
        )],
    }];
    if on {
        groups.push(wake(preferences, draft, &line));
    }
    groups.push(Group {
        heading: None,
        lines: vec![
            line(Text::SettingsVoiceChunk, Some(Text::SettingsVoiceChunkCaption), choice(chunks.to_vec())),
            line(Text::SettingsVoiceSpeaker, None, choice(voices.to_vec())),
        ],
    });
    groups
}

fn wake(
    preferences: Preferences,
    draft: Option<&(VoiceField, TextEdit)>,
    line: &impl Fn(Text<'static>, Option<Text<'static>>, Control) -> Line,
) -> Group {
    let wake = preferences.wake;
    let field = |field: Field, value: &str| {
        let written = draft.filter(|(open, _)| Some(*open) == field.voice_field()).map(|(_, edit)| edit);
        let edit = written.cloned().unwrap_or_else(|| TextEdit::new(value));
        Control::Field { edit, focused: written.is_some(), field }
    };
    let mut lines = vec![line(
        Text::SettingsWakeEnabled,
        Some(Text::SettingsWakeEnabledCaption),
        Control::Switch { on: wake.enabled, flip: Setting::WakeEnabled(!wake.enabled) },
    )];
    if wake.enabled {
        let word = field(Field::WakeWord, wake.word.as_str());
        lines.push(line(Text::SettingsWakeWord, Some(Text::SettingsWakeWordCaption), word));
    }
    let address = field(Field::VoiceAddress, preferences.voice_address.as_str());
    lines.push(line(Text::SettingsVoiceAddress, Some(Text::SettingsVoiceAddressCaption), address));
    Group { heading: None, lines }
}

fn speaker_name(voice: Voice) -> &'static str {
    match voice {
        Voice::PmSanta => "Santa",
        Voice::PmAlex => "Alex",
        Voice::PfDora => "Dora",
        Voice::AfHeart => "Heart",
        Voice::AmMichael => "Michael",
    }
}

#[cfg(test)]
mod tests {
    use superfik_i18n::Language;

    use super::*;

    fn chosen(groups: &[Group]) -> Vec<(String, String)> {
        groups
            .iter()
            .flat_map(|g| &g.lines)
            .map(|line| match &line.control {
                Control::Choice { options, selected, .. } => (line.title.clone(), options[*selected].0.clone()),
                Control::Switch { on, .. } => (line.title.clone(), on.to_string()),
                _ => (line.title.clone(), String::new()),
            })
            .collect()
    }

    fn portuguese() -> Strings {
        Strings::new(Language::Portuguese)
    }

    #[test]
    fn the_switch_comes_first_then_the_wake_word_then_response_time_and_voice() {
        let groups = groups(Preferences::default(), None, &portuguese());
        assert_eq!(
            chosen(&groups),
            [
                ("Ativar voz".to_owned(), "true".to_owned()),
                ("Palavra de ativação".to_owned(), "false".to_owned()),
                ("Como te chamar".to_owned(), String::new()),
                ("Tempo de resposta".to_owned(), "560 ms".to_owned()),
                ("Voz do assistente".to_owned(), "Santa".to_owned()),
            ]
        );
        assert_eq!(groups[0].lines[0].control.setting(0), Some(Setting::VoiceEnabled(false)));
        assert_eq!(groups[1].lines[0].control.setting(0), Some(Setting::WakeEnabled(true)));
        assert_eq!(groups[2].lines[1].detail, None, "no model names under the voice");
    }

    #[test]
    fn with_the_wake_word_on_the_word_is_written_under_it() {
        let prefs = Preferences {
            wake: superfik_voice::WakePrefs { enabled: true, ..Default::default() },
            ..Preferences::default()
        };
        let groups = groups(prefs, None, &portuguese());
        let Control::Field { edit, focused, field } = &groups[1].lines[1].control else {
            panic!("{:?}", groups[1].lines[1].control)
        };
        assert_eq!((edit.text(), *focused, *field), ("Piratinha", false, Field::WakeWord));
        let draft = (VoiceField::WakeWord, superfik_ui::TextEdit::new("Jar"));
        let editing = groups_of(prefs, Some(&draft));
        assert!(
            matches!(&editing[1].lines[1].control, Control::Field { edit, focused: true, .. } if edit.text() == "Jar")
        );
        assert!(matches!(&editing[1].lines[2].control, Control::Field { focused: false, .. }));
    }

    #[test]
    fn what_the_agent_calls_the_user_is_written_under_the_wake_word() {
        let groups = groups(Preferences::default(), None, &portuguese());
        let line = &groups[1].lines[1];
        assert_eq!(line.title, "Como te chamar");
        let Control::Field { edit, focused, field } = &line.control else { panic!("{:?}", line.control) };
        assert_eq!((edit.text(), *focused, *field), ("Capitão", false, Field::VoiceAddress));
        let draft = (VoiceField::Address, superfik_ui::TextEdit::new("Che"));
        let editing = groups_of(Preferences::default(), Some(&draft));
        assert!(
            matches!(&editing[1].lines[1].control, Control::Field { edit, focused: true, .. } if edit.text() == "Che")
        );
    }

    fn groups_of(prefs: Preferences, draft: Option<&(VoiceField, superfik_ui::TextEdit)>) -> Vec<Group> {
        groups(prefs, draft, &portuguese())
    }

    #[test]
    fn turned_off_the_wake_word_hides_and_the_choices_wait() {
        let prefs = Preferences { voice_enabled: false, ..Preferences::default() };
        let groups = groups(prefs, None, &Strings::new(Language::English));
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].lines[0].control.setting(0), Some(Setting::VoiceEnabled(true)));
        assert!(groups[1].lines.iter().all(|l| matches!(l.control, Control::Choice { enabled: false, .. })));
    }
}
