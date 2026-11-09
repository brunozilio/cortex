//! The microphone, read a step at a time and downmixed to mono, for
//! listening and for watching for the wake word alike.

use crate::VoiceProblem;

const STEP: f32 = 0.1;

pub(super) struct Input {
    mic: rodio::microphone::Microphone,
    pub rate: i32,
    channels: usize,
}

impl Input {
    pub fn open() -> Result<Self, VoiceProblem> {
        let no = |e: &dyn std::fmt::Display| VoiceProblem::NoMicrophone(e.to_string());
        let builder = rodio::microphone::MicrophoneBuilder::new().default_device().map_err(|e| no(&e))?;
        let builder = builder.default_config().map_err(|e| no(&e))?;
        let one = std::num::NonZero::new(1).ok_or_else(|| VoiceProblem::NoMicrophone("mono".into()))?;
        let builder = match builder.try_channels(one) {
            Ok(mono) => mono,
            Err(_) => builder,
        };
        let mic = builder.open_stream().map_err(|e| no(&e))?;
        let config = mic.config();
        let (rate, channels) = (config.sample_rate.get() as i32, usize::from(config.channel_count.get()));
        Ok(Self { mic, rate, channels })
    }

    pub fn read(&mut self) -> Result<Vec<f32>, VoiceProblem> {
        let frames = (self.rate as f32 * STEP) as usize;
        let mut samples = Vec::with_capacity(frames);
        let mut frame = 0.0;
        for (k, sample) in self.mic.by_ref().take(frames * self.channels).enumerate() {
            frame += sample;
            if (k + 1) % self.channels == 0 {
                samples.push(frame / self.channels as f32);
                frame = 0.0;
            }
        }
        if samples.is_empty() {
            return Err(VoiceProblem::NoMicrophone("the microphone stopped".into()));
        }
        Ok(samples)
    }
}
