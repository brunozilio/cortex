//! The voice device on this computer: models fetched into the app's data
//! folder on first use, a thread listening (microphone → Nemotron) and one
//! speaking (Kokoro → speakers). Reports reach the shell through `poll`,
//! after `wake`.

mod listen;
mod mic;
mod models;
mod speak;
mod watch;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crossbeam_channel::{Receiver, Sender};

pub use models::Models;

use crate::{ModelKind, VoiceCall, VoiceDevice, VoiceEvent};

type Report = Arc<dyn Fn(VoiceEvent) + Send + Sync>;

pub struct LocalVoice {
    models: Models,
    listen: Sender<listen::Command>,
    speak: Sender<speak::Command>,
    hushes: Arc<AtomicU64>,
    report: Report,
    events: Receiver<VoiceEvent>,
}

impl std::fmt::Debug for LocalVoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalVoice").finish_non_exhaustive()
    }
}

impl LocalVoice {
    pub fn new(root: PathBuf, wake: Arc<dyn Fn() + Send + Sync>) -> Self {
        let models = Models::new(root);
        let (sender, events) = crossbeam_channel::unbounded();
        let report: Report = Arc::new(move |event| {
            if sender.send(event).is_ok() {
                wake();
            }
        });
        let hushes = Arc::new(AtomicU64::new(0));
        let (listen, listening) = crossbeam_channel::unbounded();
        let (speak, speaking) = crossbeam_channel::unbounded();
        spawn("superfik-voice-listen", {
            let (models, report) = (models.clone(), Arc::clone(&report));
            move || listen::run(models, listening, report)
        });
        spawn("superfik-voice-speak", {
            let (models, hushes, report) = (models.clone(), Arc::clone(&hushes), Arc::clone(&report));
            move || speak::run(models, speaking, hushes, report)
        });
        Self { models, listen, speak, hushes, report, events }
    }

    fn prepare(&self, prefs: crate::VoicePrefs) {
        let (models, report) = (self.models.clone(), Arc::clone(&self.report));
        let (listen, speak) = (self.listen.clone(), self.speak.clone());
        spawn("superfik-voice-models", move || {
            let progress = |kind| {
                let report = Arc::clone(&report);
                move |fraction| report(VoiceEvent::Preparing { kind, fraction })
            };
            let fetched = models
                .fetch_recognizer(prefs.recognizer, progress(ModelKind::Recognizer))
                .and_then(|()| models.fetch_vad())
                .and_then(|()| models.fetch_kokoro(progress(ModelKind::Speech)));
            match fetched {
                Ok(()) => {
                    let _ = listen.send(listen::Command::Load(prefs.recognizer));
                    let _ = speak.send(speak::Command::Load);
                    report(VoiceEvent::Ready);
                }
                Err(problem) => report(VoiceEvent::Failed(problem)),
            }
        });
    }
}

impl VoiceDevice for LocalVoice {
    fn call(&self, call: VoiceCall) {
        let sent = match call {
            VoiceCall::Prepare(prefs) => {
                self.prepare(prefs);
                Ok(())
            }
            VoiceCall::Listen(prefs) => self.listen.send(listen::Command::Listen(prefs)).map_err(drop),
            VoiceCall::Watch { prefs, word } => self.listen.send(listen::Command::Watch(prefs, word)).map_err(drop),
            VoiceCall::Stop => self.listen.send(listen::Command::Stop).map_err(drop),
            VoiceCall::Finish => self.listen.send(listen::Command::Finish).map_err(drop),
            VoiceCall::Speak { text, voice } => {
                let generation = self.hushes.load(Ordering::SeqCst);
                self.speak.send(speak::Command::Say { text, voice, generation }).map_err(drop)
            }
            VoiceCall::Hush => {
                self.hushes.fetch_add(1, Ordering::SeqCst);
                self.speak.send(speak::Command::Hush).map_err(drop)
            }
        };
        if sent.is_err() {
            tracing::warn!("a voice thread has stopped");
        }
    }

    fn poll(&self) -> Vec<VoiceEvent> {
        self.events.try_iter().collect()
    }
}

fn spawn(name: &str, work: impl FnOnce() + Send + 'static) {
    if let Err(error) = std::thread::Builder::new().name(name.into()).spawn(work) {
        tracing::error!(%error, name, "could not start a voice thread");
    }
}

pub fn transcribe(
    root: PathBuf,
    prefs: crate::VoicePrefs,
    samples: &[f32],
    rate: i32,
) -> Result<String, crate::VoiceProblem> {
    listen::transcribe(&Models::new(root), prefs, samples, rate)
}

pub fn watch_phrases(
    root: PathBuf,
    prefs: crate::VoicePrefs,
    samples: &[f32],
    rate: i32,
) -> Result<Vec<String>, crate::VoiceProblem> {
    let models = Models::new(root);
    let mut phrases = watch::Phrases::new(&models, rate)?;
    let step = (rate / 10).max(1) as usize;
    let mut heard = Vec::new();
    for chunk in samples.chunks(step) {
        for phrase in phrases.feed(chunk) {
            heard.push(listen::transcribe(&models, prefs, &phrase, listen::RATE)?);
        }
    }
    Ok(heard)
}

pub fn synthesize(root: PathBuf, text: &str, voice: crate::Voice) -> Result<Vec<f32>, crate::VoiceProblem> {
    let models = Models::new(root);
    let mut config = sherpa_onnx::OfflineTtsConfig::default();
    let folder = models.kokoro();
    let file = |name: &str| Some(folder.join(name).to_string_lossy().into_owned());
    config.model.kokoro.model = file("model.onnx");
    config.model.kokoro.voices = file("voices.bin");
    config.model.kokoro.tokens = file("tokens.txt");
    config.model.kokoro.lexicon = file("lexicon-us-en.txt");
    config.model.num_threads = listen::threads();
    let engine = sherpa_onnx::OfflineTts::create(&config)
        .ok_or_else(|| crate::VoiceProblem::Model("Kokoro would not load".into()))?;
    Ok(speak::synthesize(&engine, text, voice))
}
