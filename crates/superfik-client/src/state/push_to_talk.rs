//! Holding left ⌘ to talk (`input::push_to_talk` decides when a hold
//! counts): the microphone opens while it is held, pauses only gather what
//! was said, and letting go has the device finish the last words and sends
//! them all. One message: once the reply is said, the voice goes back to
//! where it was (off, or watching for the wake word). A conversation
//! already on is left alone. Pure.

use superfik_voice::VoiceCall;

use super::voice::{Push, VoicePhase, VoiceState};
use super::{Effect, Session};

impl Session {
    pub(super) fn hold_talk(&mut self) -> Vec<Effect> {
        if !self.offers_voice() || self.voice.is_on() {
            return Vec::new();
        }
        self.voice = VoiceState { phase: VoicePhase::Preparing(0.0), push: Push::Held, ..VoiceState::default() };
        vec![Effect::Voice(VoiceCall::Prepare(self.preferences.voice_prefs()))]
    }

    pub(super) fn release_talk(&mut self) -> Vec<Effect> {
        if self.voice.push != Push::Held {
            return Vec::new();
        }
        match self.voice.phase {
            VoicePhase::Listening => {
                self.voice.push = Push::Released;
                vec![Effect::Voice(VoiceCall::Finish)]
            }
            _ => self.stop_voice(),
        }
    }

    pub(super) fn drop_talk(&mut self) -> Vec<Effect> {
        if self.voice.push == Push::No {
            return Vec::new();
        }
        self.stop_voice()
    }

    pub(super) fn push_heard(&mut self, text: &str, ended: bool, now: f64) -> Vec<Effect> {
        self.show_heard(text);
        if !text.trim().is_empty() {
            self.voice.said = true;
        }
        if !ended {
            return Vec::new();
        }
        if self.voice.push == Push::Held {
            self.voice.draft = self.conversation().composer.text().trim_end().to_owned();
            return Vec::new();
        }
        if !self.voice.said {
            return self.stop_voice();
        }
        self.send_heard(now)
    }
}
