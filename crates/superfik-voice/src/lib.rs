//! The voice agent: talk to the agent and hear it answer, all on this
//! computer.
//!
//! - Listening: NVIDIA Nemotron streaming speech recognition, run through
//!   sherpa-onnx.
//! - Speaking: Kokoro-82M through sherpa-onnx, fed phonemes from our own
//!   Portuguese rules (`g2p`).
//!
//! Everything but the device builds everywhere; the device (microphone,
//! models, speakers) is behind the `native` feature.

mod catalog;
mod device;
pub mod g2p;
mod short_text;
mod speech;
pub mod wake;

#[cfg(feature = "native")]
pub mod native;

pub use catalog::{
    Chunk, KOKORO_FOLDER, KOKORO_RATE, Language, Recognizer, Spoken, VAD_FILE, Voice, kokoro_url, vad_url,
};
pub use device::{ModelKind, NoVoice, Recorder, VoiceCall, VoiceDevice, VoiceEvent, VoicePrefs, VoiceProblem};
pub use short_text::Address;
pub use speech::Sentences;
pub use wake::{WakePrefs, WakeWord};
