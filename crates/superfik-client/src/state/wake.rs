//! The wake word: with it on (Settings › Voice), the microphone stays open
//! for "Piratinha" (or the word chosen) while no conversation is; saying
//! it starts one, with what followed it as the first words. A conversation
//! the word started goes back to watching after `motion::WAKE_IDLE` of
//! silence. With it off the microphone is never open in the background;
//! the mic button and `/voice` work either way. Pure.

use superfik_ui::theme::motion;
use superfik_voice::VoiceCall;

use super::voice::{Goal, VoicePhase, VoiceState};
use super::{Effect, Session, Setting};

impl Session {
    pub fn wants_watch(&self) -> bool {
        self.offers_voice() && self.preferences.wake.enabled
    }

    pub(super) fn start_watch(&mut self) -> Vec<Effect> {
        if !self.wants_watch() {
            return Vec::new();
        }
        self.voice = VoiceState { phase: VoicePhase::Preparing(0.0), goal: Goal::Watch, ..VoiceState::default() };
        vec![Effect::Voice(VoiceCall::Prepare(self.preferences.voice_prefs()))]
    }

    pub(super) fn watch(&mut self) -> Vec<Effect> {
        self.voice = VoiceState { phase: VoicePhase::Watching, ..VoiceState::default() };
        vec![self.watch_call()]
    }

    fn watch_call(&self) -> Effect {
        let prefs = self.preferences.voice_prefs();
        Effect::Voice(VoiceCall::Watch { prefs, word: self.preferences.wake.word })
    }

    pub(super) fn woke(&mut self, rest: &str, now: f64) -> Vec<Effect> {
        if self.voice.phase != VoicePhase::Watching {
            return Vec::new();
        }
        let draft = self.conversation().composer.text().trim_end().to_owned();
        self.voice = VoiceState {
            phase: VoicePhase::Listening,
            draft,
            woken: true,
            heard_at: Some(now),
            ..VoiceState::default()
        };
        let listen = Effect::Voice(VoiceCall::Listen(self.preferences.voice_prefs()));
        if rest.trim().is_empty() {
            return vec![listen];
        }
        let mut effects = self.heard(rest, true, now);
        if self.voice.phase == VoicePhase::Listening {
            effects.push(listen);
        }
        effects
    }

    pub(super) fn fell_quiet(&mut self, text: &str, now: f64) -> bool {
        let since = match self.voice.heard_at {
            Some(at) if text.trim().is_empty() => at,
            _ => {
                self.voice.heard_at = Some(now);
                return false;
            }
        };
        self.voice.woken && now - since >= motion::WAKE_IDLE
    }

    pub(super) fn voice_preferences_changed(&mut self, setting: Setting) -> Vec<Effect> {
        let off = self.voice.phase == VoicePhase::Off;
        match setting {
            Setting::VoiceEnabled(false) if !off => self.stop_voice(),
            Setting::WakeEnabled(false) if self.voice.is_watching() => {
                self.voice = VoiceState::default();
                vec![Effect::Voice(VoiceCall::Stop)]
            }
            Setting::VoiceEnabled(true) | Setting::WakeEnabled(true) if off => self.start_watch(),
            Setting::WakeWord(_) if self.voice.phase == VoicePhase::Watching => vec![self.watch_call()],
            setting if setting.reprepares_voice() && self.voice.is_watching() => self.start_watch(),
            setting if setting.reprepares_voice() => self.voice_settings_changed(),
            _ => Vec::new(),
        }
    }

    pub(super) fn voice_restored(&mut self) -> Vec<Effect> {
        if self.voice.phase == VoicePhase::Off { self.start_watch() } else { Vec::new() }
    }
}
