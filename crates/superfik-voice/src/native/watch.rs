//! Watching for the wake word: the microphone through Silero's voice
//! activity detector, so silence costs next to nothing; each phrase it
//! hears is transcribed whole by the loaded recognizer and dropped unless
//! it holds the word (`wake::find`), reported as `Woke` with what followed.

use sherpa_onnx::{LinearResampler, OnlineRecognizer, SileroVadModelConfig, VadModelConfig, VoiceActivityDetector};

use super::Report;
use super::listen::{self, RATE};
use super::mic::Input;
use super::models::Models;
use crate::wake::{self, WakeWord};
use crate::{VoiceEvent, VoicePrefs, VoiceProblem};

const WINDOW: usize = 512;
const THRESHOLD: f32 = 0.5;
const PHRASE_SILENCE: f32 = 0.6;
const SHORTEST: f32 = 0.25;
const LONGEST: f32 = 12.0;
const BUFFER: f32 = 30.0;

pub(super) struct Watching {
    input: Input,
    phrases: Phrases,
    prompt: Option<&'static str>,
    word: WakeWord,
}

pub(super) struct Phrases {
    resampler: Option<LinearResampler>,
    vad: VoiceActivityDetector,
    pending: Vec<f32>,
}

impl Phrases {
    pub fn new(models: &Models, rate: i32) -> Result<Self, VoiceProblem> {
        if !models.has_vad() {
            return Err(VoiceProblem::Model("the voice activity detector is not downloaded".into()));
        }
        let config = VadModelConfig {
            silero_vad: SileroVadModelConfig {
                model: Some(models.vad().to_string_lossy().into_owned()),
                threshold: THRESHOLD,
                min_silence_duration: PHRASE_SILENCE,
                min_speech_duration: SHORTEST,
                window_size: WINDOW as i32,
                max_speech_duration: LONGEST,
            },
            sample_rate: RATE,
            num_threads: 1,
            ..VadModelConfig::default()
        };
        let vad = VoiceActivityDetector::create(&config, BUFFER)
            .ok_or_else(|| VoiceProblem::Model("the voice activity detector would not load".into()))?;
        let resampler = if rate == RATE { None } else { LinearResampler::create(rate, RATE) };
        Ok(Self { resampler, vad, pending: Vec::new() })
    }

    pub fn feed(&mut self, samples: &[f32]) -> Vec<Vec<f32>> {
        match &self.resampler {
            Some(resampler) => self.pending.extend(resampler.resample(samples, false)),
            None => self.pending.extend_from_slice(samples),
        }
        let whole = self.pending.len() / WINDOW * WINDOW;
        for window in self.pending[..whole].as_chunks::<WINDOW>().0 {
            self.vad.accept_waveform(window);
        }
        self.pending.drain(..whole);
        let mut phrases = Vec::new();
        while let Some(phrase) = self.vad.front() {
            phrases.push(phrase.samples().to_vec());
            self.vad.pop();
        }
        phrases
    }

    pub fn clear(&mut self) {
        self.vad.clear();
    }
}

pub(super) fn open(models: &Models, prefs: VoicePrefs, word: WakeWord) -> Result<Watching, VoiceProblem> {
    let input = Input::open()?;
    let phrases = Phrases::new(models, input.rate)?;
    let prompt = prefs.recognizer.prompt(prefs.heard_language());
    tracing::info!(rate = input.rate, word = word.as_str(), "watching for the wake word");
    Ok(Watching { input, phrases, prompt, word })
}

pub(super) fn step(watching: &mut Watching, engine: &OnlineRecognizer, report: &Report) -> Result<(), VoiceProblem> {
    let samples = watching.input.read()?;
    for phrase in watching.phrases.feed(&samples) {
        let text = listen::decode(engine, watching.prompt, &phrase, RATE);
        tracing::debug!(%text, "heard while watching");
        if let Some(rest) = wake::find(&text, watching.word) {
            report(VoiceEvent::Woke { rest });
            watching.phrases.clear();
            return Ok(());
        }
    }
    Ok(())
}
