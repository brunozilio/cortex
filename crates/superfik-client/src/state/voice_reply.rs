//! The reply, said aloud: each sentence goes to the speakers as it
//! streams in; once it is all said, the voice listens again (or ends a
//! left-⌘ exchange). Cancelling the run hushes it. Pure.

use superfik_core::{ConversationId, EngineEvent};
use superfik_voice::{Sentences, VoiceCall};

use super::voice::VoicePhase;
use super::{Effect, Session};

impl Session {
    pub(super) fn voice_run_event(&mut self, id: ConversationId, event: &EngineEvent) -> Vec<Effect> {
        let waiting = matches!(self.voice.phase, VoicePhase::Waiting | VoicePhase::Speaking);
        if !waiting || self.voice.conversation != Some(id) {
            return Vec::new();
        }
        let sentences = match event {
            EngineEvent::TextDelta(delta) => self.voice.reply.push(delta),
            event if event.is_terminal() => {
                self.voice.replied = true;
                self.voice.reply.finish()
            }
            _ => return Vec::new(),
        };
        let voice = self.preferences.voice.voice;
        let mut effects: Vec<Effect> =
            sentences.into_iter().map(|text| Effect::Voice(VoiceCall::Speak { text, voice })).collect();
        if !effects.is_empty() {
            self.voice.phase = VoicePhase::Speaking;
            self.voice.speaking = true;
        }
        if self.voice.replied && !self.voice.speaking {
            effects.extend(self.after_reply());
        }
        effects
    }

    pub(super) fn hush_voice(&mut self) -> Vec<Effect> {
        if !matches!(self.voice.phase, VoicePhase::Waiting | VoicePhase::Speaking) {
            return Vec::new();
        }
        self.voice.reply = Sentences::default();
        self.voice.speaking = false;
        vec![Effect::Voice(VoiceCall::Hush)]
    }
}
