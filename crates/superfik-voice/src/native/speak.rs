//! The speaking thread: each sentence through Kokoro-82M into a queue on the
//! speakers, as soon as the one before was synthesized, so the voice runs
//! ahead of the reply without waiting for it to end.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use crossbeam_channel::{Receiver, RecvTimeoutError};
use sherpa_onnx::{GenerationConfig, OfflineTts, OfflineTtsConfig};

use super::Report;
use super::models::Models;
use crate::{KOKORO_RATE, Spoken, Voice, VoiceEvent, VoiceProblem, g2p};

const TICK: Duration = Duration::from_millis(40);

pub(super) enum Command {
    Load,
    Say { text: String, voice: Voice, generation: u64 },
    Hush,
}

struct Output {
    _sink: rodio::MixerDeviceSink,
    player: rodio::Player,
}

fn open() -> Result<Output, VoiceProblem> {
    use rodio::cpal::traits::{DeviceTrait, HostTrait};

    let mut sink = rodio::DeviceSinkBuilder::from_default_device()
        .and_then(|builder| builder.with_error_callback(output_event).open_sink_or_fallback())
        .or_else(|original| {
            let Ok(devices) = rodio::cpal::default_host().output_devices() else {
                return Err(original);
            };
            devices
                .filter(|device| {
                    device.description().is_ok_and(|description| description.driver().is_some_and(|d| d != "null"))
                })
                .find_map(|device| {
                    rodio::DeviceSinkBuilder::from_device(device)
                        .and_then(|builder| builder.with_error_callback(output_event).open_sink_or_fallback())
                        .ok()
                })
                .ok_or(original)
        })
        .map_err(|e| VoiceProblem::Model(format!("no speakers: {e}")))?;
    sink.log_on_drop(false);
    let player = rodio::Player::connect_new(sink.mixer());
    Ok(Output { _sink: sink, player })
}

fn output_event(error: rodio::cpal::Error) {
    if error.kind() == rodio::cpal::ErrorKind::DeviceChanged {
        tracing::debug!(%error, "voice output changed");
    } else {
        tracing::error!(%error, "voice audio stream error");
    }
}

pub(super) fn run(models: Models, commands: Receiver<Command>, hushes: Arc<AtomicU64>, report: Report) {
    let mut kokoro: Option<OfflineTts> = None;
    let mut output: Option<Output> = None;
    let mut queued = false;
    loop {
        let next = if queued {
            commands.recv_timeout(TICK)
        } else {
            commands.recv().map_err(|_| RecvTimeoutError::Disconnected)
        };
        match next {
            Ok(Command::Load) => {
                let ready = load(&models, &mut kokoro).and_then(|()| speakers(&mut output));
                if let Err(problem) = ready {
                    report(VoiceEvent::Failed(problem));
                }
            }
            Ok(Command::Say { text, voice, generation }) => {
                if generation < hushes.load(Ordering::SeqCst) {
                    continue;
                }
                if let Err(problem) = load(&models, &mut kokoro).and_then(|()| speakers(&mut output)) {
                    report(VoiceEvent::Failed(problem));
                    continue;
                }
                let (Some(engine), Some(out)) = (&kokoro, &output) else { continue };
                let samples = synthesize(engine, &text, voice);
                if generation < hushes.load(Ordering::SeqCst) || samples.is_empty() {
                    continue;
                }
                if let (Some(channels), Some(rate)) = (std::num::NonZero::new(1), std::num::NonZero::new(KOKORO_RATE)) {
                    out.player.append(rodio::buffer::SamplesBuffer::new(channels, rate, samples));
                    out.player.play();
                    queued = true;
                }
            }
            Ok(Command::Hush) => {
                if let Some(out) = &output {
                    out.player.clear();
                    out.player.play();
                }
                queued = false;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        if queued && output.as_ref().is_none_or(|o| o.player.empty()) && commands.is_empty() {
            queued = false;
            report(VoiceEvent::Spoken);
        }
    }
}

fn speakers(output: &mut Option<Output>) -> Result<(), VoiceProblem> {
    if output.is_none() {
        *output = Some(open()?);
    }
    Ok(())
}

fn load(models: &Models, kokoro: &mut Option<OfflineTts>) -> Result<(), VoiceProblem> {
    if kokoro.is_some() {
        return Ok(());
    }
    if !models.has_kokoro() {
        return Err(VoiceProblem::Model("Kokoro is not downloaded".into()));
    }
    let folder = models.kokoro();
    let file = |name: &str| Some(folder.join(name).to_string_lossy().into_owned());
    let mut config = OfflineTtsConfig::default();
    config.model.kokoro.model = file("model.onnx");
    config.model.kokoro.voices = file("voices.bin");
    config.model.kokoro.tokens = file("tokens.txt");
    config.model.kokoro.lexicon = file("lexicon-us-en.txt");
    config.model.num_threads = super::listen::threads();
    let started = std::time::Instant::now();
    let engine = OfflineTts::create(&config).ok_or_else(|| VoiceProblem::Model("Kokoro would not load".into()))?;
    tracing::info!(took = ?started.elapsed(), "Kokoro loaded");
    *kokoro = Some(engine);
    Ok(())
}

pub(super) fn synthesize(engine: &OfflineTts, text: &str, voice: Voice) -> Vec<f32> {
    let mut generation = GenerationConfig { sid: voice.speaker(), ..Default::default() };
    if voice.speaks() == Spoken::Portuguese {
        let points = g2p::codepoints(text);
        if points.is_empty() {
            return Vec::new();
        }
        generation.phoneme_codepoints = Some(vec![points]);
    }
    let started = std::time::Instant::now();
    let audio = engine.generate_with_config::<fn(&[f32], f32) -> bool>(text, &generation, None);
    tracing::debug!(took = ?started.elapsed(), chars = text.len(), "sentence synthesized");
    audio.map(|a| a.samples().to_vec()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    #[test]
    fn rerouting_is_debug_but_output_failures_remain_errors() {
        use std::sync::{Arc, Mutex};
        use tracing_subscriber::{Layer, layer::SubscriberExt};

        struct Levels(Arc<Mutex<Vec<tracing::Level>>>);
        impl<S: tracing::Subscriber> Layer<S> for Levels {
            fn on_event(&self, event: &tracing::Event<'_>, _: tracing_subscriber::layer::Context<'_, S>) {
                self.0.lock().expect("log levels lock").push(*event.metadata().level());
            }
        }
        let levels = Arc::new(Mutex::new(Vec::new()));
        let subscriber = tracing_subscriber::registry().with(Levels(levels.clone()));
        tracing::subscriber::with_default(subscriber, || {
            super::output_event(rodio::cpal::Error::with_message(
                rodio::cpal::ErrorKind::DeviceChanged,
                "route switched",
            ));
            super::output_event(rodio::cpal::Error::with_message(
                rodio::cpal::ErrorKind::DeviceNotAvailable,
                "no output",
            ));
            super::output_event(rodio::cpal::Error::with_message(
                rodio::cpal::ErrorKind::StreamInvalidated,
                "stream stopped",
            ));
        });
        assert_eq!(
            *levels.lock().expect("log levels lock"),
            [tracing::Level::DEBUG, tracing::Level::ERROR, tracing::Level::ERROR]
        );
    }
}
