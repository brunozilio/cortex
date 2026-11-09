//! Values typed in place on the Settings screen (`rows::Control::Field`).

use crate::state::ServerField;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    WakeWord,
    VoiceAddress,
    Server(ServerField),
}

impl Field {
    pub fn placeholder(self) -> &'static str {
        match self {
            Field::WakeWord => superfik_voice::WakeWord::DEFAULT,
            Field::VoiceAddress => superfik_voice::Address::DEFAULT,
            Field::Server(field) => field.placeholder(),
        }
    }

    pub fn secret(self) -> bool {
        matches!(self, Field::Server(field) if field.secret())
    }

    pub fn voice_field(self) -> Option<crate::state::VoiceField> {
        match self {
            Field::WakeWord => Some(crate::state::VoiceField::WakeWord),
            Field::VoiceAddress => Some(crate::state::VoiceField::Address),
            Field::Server(_) => None,
        }
    }
}
