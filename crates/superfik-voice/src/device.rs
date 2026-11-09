//! What the reducer asks of the voice device and what the device reports.
//! Pure vocabulary; the device itself is `native::LocalVoice` (or a
//! stand-in).

use serde::{Deserialize, Serialize};

use crate::{Language, Recognizer, Voice};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct VoicePrefs {
    pub recognizer: Recognizer,
    pub language: Language,
    pub voice: Voice,
}

impl VoicePrefs {
    pub fn heard_language(self) -> Language {
        if self.recognizer.understands(self.language) { self.language } else { Language::English }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum VoiceCall {
    Prepare(VoicePrefs),
    Listen(VoicePrefs),
    Watch { prefs: VoicePrefs, word: crate::WakeWord },
    Finish,
    Stop,
    Speak { text: String, voice: Voice },
    Hush,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelKind {
    Recognizer,
    Speech,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoiceProblem {
    NoMicrophone(String),
    Download(String),
    Model(String),
}

impl std::fmt::Display for VoiceProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoMicrophone(why) => write!(f, "microphone: {why}"),
            Self::Download(why) => write!(f, "download: {why}"),
            Self::Model(why) => write!(f, "model: {why}"),
        }
    }
}

impl std::error::Error for VoiceProblem {}

#[derive(Debug, Clone, PartialEq)]
pub enum VoiceEvent {
    Preparing { kind: ModelKind, fraction: f32 },
    Ready,
    Heard { text: String, ended: bool },
    Woke { rest: String },
    Spoken,
    Failed(VoiceProblem),
}

pub trait VoiceDevice: Send {
    fn call(&self, call: VoiceCall);
    fn poll(&self) -> Vec<VoiceEvent>;
}

#[derive(Debug, Default)]
pub struct NoVoice {
    failures: std::sync::Mutex<Vec<VoiceEvent>>,
}

impl VoiceDevice for NoVoice {
    fn call(&self, call: VoiceCall) {
        if matches!(call, VoiceCall::Prepare(_) | VoiceCall::Listen(_) | VoiceCall::Watch { .. }) {
            let problem = VoiceProblem::Model("this build has no voice".into());
            self.failures.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(VoiceEvent::Failed(problem));
        }
    }

    fn poll(&self) -> Vec<VoiceEvent> {
        std::mem::take(&mut *self.failures.lock().unwrap_or_else(std::sync::PoisonError::into_inner))
    }
}

#[derive(Debug, Default, Clone)]
pub struct Recorder {
    calls: std::sync::Arc<std::sync::Mutex<Vec<VoiceCall>>>,
    events: std::sync::Arc<std::sync::Mutex<Vec<VoiceEvent>>>,
}

impl Recorder {
    pub fn calls(&self) -> Vec<VoiceCall> {
        self.calls.lock().map(|c| c.clone()).unwrap_or_default()
    }

    pub fn report(&self, event: VoiceEvent) {
        if let Ok(mut events) = self.events.lock() {
            events.push(event);
        }
    }
}

impl VoiceDevice for Recorder {
    fn call(&self, call: VoiceCall) {
        if let Ok(mut calls) = self.calls.lock() {
            calls.push(call);
        }
    }

    fn poll(&self) -> Vec<VoiceEvent> {
        self.events.lock().map(|mut e| std::mem::take(&mut *e)).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Chunk;

    #[test]
    fn the_english_checkpoint_hears_english_whatever_was_chosen() {
        let prefs = VoicePrefs { recognizer: Recognizer::NemotronEnglish(Chunk::Ms560), ..VoicePrefs::default() };
        assert_eq!(prefs.heard_language(), Language::English);
        assert_eq!(VoicePrefs::default().heard_language(), Language::PortugueseBrazil);
    }

    #[test]
    fn no_voice_answers_every_prepare_listen_and_watch_with_a_failure() {
        let device = NoVoice::default();
        let prefs = VoicePrefs::default();
        device.call(VoiceCall::Prepare(prefs));
        device.call(VoiceCall::Listen(prefs));
        device.call(VoiceCall::Watch { prefs, word: crate::WakeWord::default() });
        device.call(VoiceCall::Stop);
        let events = device.poll();
        assert_eq!(events.len(), 3, "{events:?}");
        assert!(events.iter().all(|e| matches!(e, VoiceEvent::Failed(_))));
        assert!(device.poll().is_empty(), "reported once");
    }

    #[test]
    fn missing_fields_take_their_defaults() {
        let prefs: VoicePrefs = serde_json::from_str(r#"{"voice":"pf_dora"}"#).expect("test");
        assert_eq!(prefs, VoicePrefs { voice: Voice::PfDora, ..VoicePrefs::default() });
    }
}
