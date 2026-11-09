//! Where clips go: the speakers, nowhere, or a test recorder.

use std::io::Cursor;
use std::sync::{Arc, Mutex};

use rodio::Source;

pub trait Player: std::fmt::Debug + Send {
    fn play(&self, clip: &'static [u8], gain: f32);
}

pub struct Speakers {
    sink: rodio::MixerDeviceSink,
}

impl std::fmt::Debug for Speakers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Speakers").finish_non_exhaustive()
    }
}

impl Speakers {
    pub fn open() -> Result<Self, rodio::DeviceSinkError> {
        let builder = rodio::DeviceSinkBuilder::from_default_device()?
            .with_buffer_size(rodio::cpal::BufferSize::Default)
            .with_error_callback(output_event);
        let mut sink = match builder.open_stream() {
            Ok(sink) => sink,
            Err(error) => {
                tracing::debug!(?error, "default stream config refused; trying fallbacks");
                rodio::DeviceSinkBuilder::from_default_device()?
                    .with_error_callback(output_event)
                    .open_sink_or_fallback()?
            }
        };
        sink.log_on_drop(false);
        Ok(Self { sink })
    }
}

fn output_event(error: rodio::cpal::Error) {
    if error.kind() == rodio::cpal::ErrorKind::DeviceChanged {
        tracing::debug!(%error, "audio output changed");
    } else {
        tracing::error!(%error, "audio stream error");
    }
}

impl Player for Speakers {
    fn play(&self, clip: &'static [u8], gain: f32) {
        match rodio::Decoder::new(Cursor::new(clip)) {
            Ok(source) => self.sink.mixer().add(source.amplify(gain)),
            Err(error) => tracing::warn!(%error, "could not decode sound clip"),
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Silent;

impl Player for Silent {
    fn play(&self, _clip: &'static [u8], _gain: f32) {}
}

#[derive(Debug, Default, Clone)]
pub struct Recorder {
    played: Arc<Mutex<Vec<&'static [u8]>>>,
}

impl Recorder {
    pub fn played(&self) -> Vec<&'static [u8]> {
        self.played.lock().map(|p| p.clone()).unwrap_or_default()
    }
}

impl Player for Recorder {
    fn play(&self, clip: &'static [u8], _gain: f32) {
        if let Ok(mut played) = self.played.lock() {
            played.push(clip);
        }
    }
}

#[cfg(test)]
mod tests {
    use rodio::Source;

    use crate::{Cue, PackId};

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

    #[test]
    fn every_clip_decodes_without_a_device() {
        for id in PackId::ALL {
            for cue in Cue::ALL {
                for clip in id.pack().clips(cue) {
                    let source = rodio::Decoder::new(std::io::Cursor::new(*clip)).expect("decodes");
                    assert_eq!(source.channels().get(), 1, "{id}: {} is not mono", cue.name());
                    assert!(source.count() > 0, "{id}: {} is empty", cue.name());
                }
            }
        }
    }
}
