//! One choice made on the Settings screen. Pure.

use super::preferences::{Language, SoundChoice, Theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Setting {
    Sound(SoundChoice),
    Haptics(bool),
    Language(Language),
    Theme(Theme),
    FillOnLoad(bool),
    Lock(bool),
    Policy(super::governance::PolicyChoice),
    Team(super::teams::TeamChoice),
    VoiceEnabled(bool),
    VoiceChunk(superfik_voice::Chunk),
    VoiceSpeaker(superfik_voice::Voice),
    WakeEnabled(bool),
    WakeWord(superfik_voice::WakeWord),
    VoiceAddress(superfik_voice::Address),
    OutputStyle(superfik_core::OutputStyle),
    Suggestions(bool),
    SafetyCheck(bool),
    HeadsUp(bool),
}

impl Setting {
    pub const fn reprepares_voice(self) -> bool {
        matches!(self, Self::VoiceChunk(_) | Self::VoiceSpeaker(_) | Self::Language(_))
    }
}
