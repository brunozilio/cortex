//! The listening thread: the microphone into a Nemotron streaming
//! recognizer, reporting what was heard as it firms up and when the user
//! paused (sherpa-onnx's endpoint rules), including a pause with nothing
//! said, so the reducer can tell silence lasted. The same thread watches
//! for the wake word (`watch`): one microphone, one mode at a time.

use std::path::Path;

use crossbeam_channel::Receiver;
use sherpa_onnx::{OnlineRecognizer, OnlineRecognizerConfig, OnlineStream};

use super::Report;
use super::mic::Input;
use super::models::Models;
use super::watch::{self, Watching};
use crate::{Recognizer, VoiceEvent, VoicePrefs, VoiceProblem, WakeWord};

const IDLE_SILENCE: f32 = 2.4;
const TURN_SILENCE: f32 = 1.0;
const LONGEST: f32 = 25.0;
const LEAD_IN: f32 = 0.5;
pub(super) const RATE: i32 = 16_000;

pub(super) enum Command {
    Load(Recognizer),
    Listen(VoicePrefs),
    Watch(VoicePrefs, WakeWord),
    Finish,
    Stop,
}

struct Loaded {
    recognizer: Recognizer,
    engine: OnlineRecognizer,
}

struct Open {
    input: Input,
    stream: OnlineStream,
    prompt: Option<&'static str>,
    heard: String,
}

enum Mic {
    Listen(Open),
    Watch(Watching),
}

pub(super) fn run(models: Models, commands: Receiver<Command>, report: Report) {
    let mut loaded: Option<Loaded> = None;
    let mut open: Option<Mic> = None;
    loop {
        let command = match &open {
            None => match commands.recv() {
                Ok(command) => Some(command),
                Err(_) => return,
            },
            Some(_) => commands.try_recv().ok(),
        };
        match command {
            Some(Command::Load(recognizer)) => {
                if let Err(problem) = load(&models, &mut loaded, recognizer) {
                    report(VoiceEvent::Failed(problem));
                }
            }
            Some(Command::Listen(prefs)) => {
                open = None;
                match listen(&models, &mut loaded, prefs) {
                    Ok(listening) => open = Some(Mic::Listen(listening)),
                    Err(problem) => report(VoiceEvent::Failed(problem)),
                }
            }
            Some(Command::Watch(prefs, word)) => {
                open = None;
                let opened = ready(&models, &mut loaded, prefs).and_then(|()| watch::open(&models, prefs, word));
                match opened {
                    Ok(watching) => open = Some(Mic::Watch(watching)),
                    Err(problem) => report(VoiceEvent::Failed(problem)),
                }
            }
            Some(Command::Finish) => {
                if let (Some(Mic::Listen(listening)), Some(engine)) = (&mut open, &loaded) {
                    finish(listening, &engine.engine, &report);
                }
            }
            Some(Command::Stop) => open = None,
            None => {}
        }
        let Some(engine) = &loaded else { continue };
        let stepped = match &mut open {
            Some(Mic::Listen(listening)) => step(listening, &engine.engine, &report),
            Some(Mic::Watch(watching)) => watch::step(watching, &engine.engine, &report),
            None => Ok(()),
        };
        if let Err(problem) = stepped {
            open = None;
            report(VoiceEvent::Failed(problem));
        }
    }
}

fn ready(models: &Models, loaded: &mut Option<Loaded>, prefs: VoicePrefs) -> Result<(), VoiceProblem> {
    if !models.has_recognizer(prefs.recognizer) {
        return Err(VoiceProblem::Model(format!("{} is not downloaded", prefs.recognizer.checkpoint())));
    }
    load(models, loaded, prefs.recognizer)
}

fn load(models: &Models, loaded: &mut Option<Loaded>, recognizer: Recognizer) -> Result<(), VoiceProblem> {
    if loaded.as_ref().is_some_and(|l| l.recognizer == recognizer) {
        return Ok(());
    }
    *loaded = None;
    let started = std::time::Instant::now();
    let engine = engine(&models.recognizer(recognizer))
        .ok_or_else(|| VoiceProblem::Model(format!("{} would not load", recognizer.checkpoint())))?;
    tracing::info!(checkpoint = recognizer.checkpoint(), took = ?started.elapsed(), "speech recognizer loaded");
    *loaded = Some(Loaded { recognizer, engine });
    Ok(())
}

fn engine(folder: &Path) -> Option<OnlineRecognizer> {
    let file = |name: &str| Some(folder.join(name).to_string_lossy().into_owned());
    let mut config = OnlineRecognizerConfig::default();
    config.model_config.transducer.encoder = file("encoder.int8.onnx");
    config.model_config.transducer.decoder = file("decoder.int8.onnx");
    config.model_config.transducer.joiner = file("joiner.int8.onnx");
    config.model_config.tokens = file("tokens.txt");
    config.model_config.num_threads = threads();
    config.decoding_method = Some("greedy_search".into());
    config.enable_endpoint = true;
    config.rule1_min_trailing_silence = IDLE_SILENCE;
    config.rule2_min_trailing_silence = TURN_SILENCE;
    config.rule3_min_utterance_length = LONGEST;
    OnlineRecognizer::create(&config)
}

pub(super) fn transcribe(
    models: &Models,
    prefs: VoicePrefs,
    samples: &[f32],
    rate: i32,
) -> Result<String, VoiceProblem> {
    let mut loaded = None;
    load(models, &mut loaded, prefs.recognizer)?;
    let engine = &loaded.as_ref().ok_or_else(|| VoiceProblem::Model("no recognizer".into()))?.engine;
    Ok(decode(engine, prefs.recognizer.prompt(prefs.heard_language()), samples, rate))
}

pub(super) fn decode(engine: &OnlineRecognizer, prompt: Option<&str>, samples: &[f32], rate: i32) -> String {
    let stream = fresh(engine, prompt, rate);
    stream.accept_waveform(rate, samples);
    stream.accept_waveform(rate, &silence(rate, 1.0));
    stream.input_finished();
    while engine.is_ready(&stream) {
        engine.decode(&stream);
    }
    engine.get_result(&stream).map(|r| r.text.trim().to_owned()).unwrap_or_default()
}

pub(super) fn threads() -> i32 {
    let cores = std::thread::available_parallelism().map_or(4, std::num::NonZero::get);
    (cores / 2).clamp(2, 4) as i32
}

fn listen(models: &Models, loaded: &mut Option<Loaded>, prefs: VoicePrefs) -> Result<Open, VoiceProblem> {
    ready(models, loaded, prefs)?;
    let input = Input::open()?;
    let engine = &loaded.as_ref().ok_or_else(|| VoiceProblem::Model("no recognizer".into()))?.engine;
    let prompt = prefs.recognizer.prompt(prefs.heard_language());
    let stream = fresh(engine, prompt, input.rate);
    tracing::info!(rate = input.rate, ?prompt, "listening");
    Ok(Open { input, stream, prompt, heard: String::new() })
}

fn fresh(engine: &OnlineRecognizer, prompt: Option<&str>, rate: i32) -> OnlineStream {
    let stream = engine.create_stream();
    if let Some(language) = prompt {
        stream.set_option("language", language);
    }
    stream.accept_waveform(rate, &silence(rate, LEAD_IN));
    stream
}

fn silence(rate: i32, seconds: f32) -> Vec<f32> {
    vec![0.0; (rate as f32 * seconds) as usize]
}

fn finish(open: &mut Open, engine: &OnlineRecognizer, report: &Report) {
    let rate = open.input.rate;
    open.stream.accept_waveform(rate, &silence(rate, LEAD_IN));
    open.stream.input_finished();
    while engine.is_ready(&open.stream) {
        engine.decode(&open.stream);
    }
    let text = engine.get_result(&open.stream).map(|r| r.text.trim().to_owned()).unwrap_or_default();
    report(VoiceEvent::Heard { text, ended: true });
    open.stream = fresh(engine, open.prompt, rate);
    open.heard.clear();
}

fn step(open: &mut Open, engine: &OnlineRecognizer, report: &Report) -> Result<(), VoiceProblem> {
    let samples = open.input.read()?;
    open.stream.accept_waveform(open.input.rate, &samples);
    while engine.is_ready(&open.stream) {
        engine.decode(&open.stream);
    }
    let text = engine.get_result(&open.stream).map(|r| r.text.trim().to_owned()).unwrap_or_default();
    let ended = engine.is_endpoint(&open.stream);
    if ended || text != open.heard {
        report(VoiceEvent::Heard { text: text.clone(), ended });
        open.heard = text;
    }
    if ended {
        open.stream = fresh(engine, open.prompt, open.input.rate);
        open.heard.clear();
    }
    Ok(())
}
