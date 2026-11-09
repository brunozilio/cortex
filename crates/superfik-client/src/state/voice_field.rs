//! Writing in Settings › Voice: the wake word and what the agent calls the
//! user. The field edits a copy; Enter, another field, another pane or
//! closing Settings keeps it, Escape drops it. An emptied one goes back to
//! its default. Pure.

use superfik_ui::TextEdit;
use superfik_voice::{Address, WakeWord};

use super::{EditKey, Effect, Session, Setting};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceField {
    WakeWord,
    Address,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VoiceFieldAction {
    Focus(VoiceField),
    Edit(EditKey),
    Save,
    Cancel,
}

impl Session {
    pub(super) fn voice_field(&mut self, action: VoiceFieldAction) -> Vec<Effect> {
        match action {
            VoiceFieldAction::Focus(field) => {
                if self.voice_field_edit.as_ref().is_some_and(|(open, _)| *open == field) {
                    return Vec::new();
                }
                let effects = self.keep_voice_field();
                let value = match field {
                    VoiceField::WakeWord => self.preferences.wake.word.as_str(),
                    VoiceField::Address => self.preferences.voice_address.as_str(),
                };
                self.voice_field_edit = Some((field, TextEdit::new(value)));
                effects
            }
            VoiceFieldAction::Edit(key) => {
                if let Some((_, edit)) = &mut self.voice_field_edit {
                    super::composer::apply_key(edit, key);
                }
                Vec::new()
            }
            VoiceFieldAction::Save => self.keep_voice_field(),
            VoiceFieldAction::Cancel => {
                self.voice_field_edit = None;
                Vec::new()
            }
        }
    }

    pub(super) fn keep_voice_field(&mut self) -> Vec<Effect> {
        let Some((field, edit)) = self.voice_field_edit.take() else { return Vec::new() };
        let setting = match field {
            VoiceField::WakeWord => Setting::WakeWord(WakeWord::new(edit.text()).unwrap_or_default()),
            VoiceField::Address => Setting::VoiceAddress(Address::new(edit.text()).unwrap_or_default()),
        };
        self.choose_setting(setting)
    }
}
