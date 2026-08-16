use rodio::buffer::SamplesBuffer;
use rodio::{OutputStream, OutputStreamHandle, Source};

const SAMPLE_RATE: u32 = 44_100;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum SoundEffect {
    BuildPlaced,
    ConstructionComplete,
    LinkConnected,
    LinkDisconnected,
    UpgradeStarted,
    UpgradeComplete,
    AttackBlocked,
    Breach,
    ObjectiveComplete,
    Victory,
    Error,
    Pause,
    Resume,
    Toggle,
}

struct AudioBackend {
    _stream: OutputStream,
    handle: OutputStreamHandle,
}

pub(crate) struct SoundEngine {
    backend: Option<AudioBackend>,
}

impl SoundEngine {
    pub(crate) fn new() -> Self {
        let backend = OutputStream::try_default()
            .ok()
            .map(|(stream, handle)| AudioBackend {
                _stream: stream,
                handle,
            });
        Self { backend }
    }

    pub(crate) fn play(&self, effect: SoundEffect, volume: f32) {
        let Some(backend) = &self.backend else {
            return;
        };
        let source = SamplesBuffer::new(1, SAMPLE_RATE, synthesize(effect)).amplify(volume);
        let _ = backend.handle.play_raw(source);
    }
}

fn synthesize(effect: SoundEffect) -> Vec<f32> {
    let mut output = Vec::new();
    match effect {
        SoundEffect::BuildPlaced => {
            tone(&mut output, 220.0, 45, 0.16);
            tone(&mut output, 330.0, 65, 0.14);
        }
        SoundEffect::ConstructionComplete => {
            for frequency in [330.0, 440.0, 660.0] {
                tone(&mut output, frequency, 48, 0.14);
            }
        }
        SoundEffect::LinkConnected => {
            tone(&mut output, 520.0, 35, 0.11);
            rest(&mut output, 12);
            tone(&mut output, 780.0, 55, 0.13);
        }
        SoundEffect::LinkDisconnected => {
            tone(&mut output, 620.0, 38, 0.12);
            tone(&mut output, 310.0, 70, 0.13);
        }
        SoundEffect::UpgradeStarted => {
            for frequency in [260.0, 390.0, 520.0, 780.0] {
                tone(&mut output, frequency, 42, 0.13);
                rest(&mut output, 8);
            }
        }
        SoundEffect::UpgradeComplete => {
            for frequency in [440.0, 660.0, 880.0, 1_100.0] {
                tone(&mut output, frequency, 50, 0.14);
            }
        }
        SoundEffect::AttackBlocked => {
            tone(&mut output, 880.0, 45, 0.14);
            rest(&mut output, 18);
            tone(&mut output, 1_180.0, 85, 0.16);
        }
        SoundEffect::Breach => {
            tone(&mut output, 170.0, 95, 0.18);
            noise(&mut output, 210, 0.18);
            tone(&mut output, 110.0, 120, 0.15);
        }
        SoundEffect::ObjectiveComplete => {
            tone(&mut output, 660.0, 48, 0.13);
            tone(&mut output, 880.0, 85, 0.15);
        }
        SoundEffect::Victory => {
            for frequency in [440.0, 554.0, 660.0, 880.0] {
                tone(&mut output, frequency, 90, 0.16);
                rest(&mut output, 15);
            }
            tone(&mut output, 1_320.0, 180, 0.17);
        }
        SoundEffect::Error => {
            tone(&mut output, 145.0, 65, 0.16);
            rest(&mut output, 18);
            tone(&mut output, 120.0, 95, 0.17);
        }
        SoundEffect::Pause => {
            tone(&mut output, 440.0, 50, 0.11);
            tone(&mut output, 300.0, 80, 0.12);
        }
        SoundEffect::Resume => {
            tone(&mut output, 300.0, 50, 0.11);
            tone(&mut output, 440.0, 80, 0.12);
        }
        SoundEffect::Toggle => tone(&mut output, 520.0, 70, 0.12),
    }
    output
}

fn tone(output: &mut Vec<f32>, frequency: f32, milliseconds: u32, volume: f32) {
    let count = samples_for(milliseconds);
    for index in 0..count {
        let phase = (index as f32 * frequency / SAMPLE_RATE as f32).fract();
        let square = if phase < 0.5 { 1.0 } else { -1.0 };
        let attack = (index as f32 / 48.0).min(1.0);
        let release = ((count - index) as f32 / 120.0).min(1.0);
        output.push(square * volume * attack * release);
    }
}

fn rest(output: &mut Vec<f32>, milliseconds: u32) {
    output.resize(output.len() + samples_for(milliseconds), 0.0);
}

fn noise(output: &mut Vec<f32>, milliseconds: u32, volume: f32) {
    let count = samples_for(milliseconds);
    let mut lfsr = 0xACE1_u16;
    for index in 0..count {
        let bit = (lfsr ^ (lfsr >> 1)) & 1;
        lfsr = (lfsr >> 1) | (bit << 15);
        let sample = if lfsr & 1 == 0 { -1.0 } else { 1.0 };
        let envelope = 1.0 - index as f32 / count as f32;
        output.push(sample * volume * envelope);
    }
}

const fn samples_for(milliseconds: u32) -> usize {
    (SAMPLE_RATE as u64 * milliseconds as u64 / 1_000) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_effect_generates_bounded_non_silent_samples() {
        let effects = [
            SoundEffect::BuildPlaced,
            SoundEffect::ConstructionComplete,
            SoundEffect::LinkConnected,
            SoundEffect::LinkDisconnected,
            SoundEffect::UpgradeStarted,
            SoundEffect::UpgradeComplete,
            SoundEffect::AttackBlocked,
            SoundEffect::Breach,
            SoundEffect::ObjectiveComplete,
            SoundEffect::Victory,
            SoundEffect::Error,
            SoundEffect::Pause,
            SoundEffect::Resume,
            SoundEffect::Toggle,
        ];
        for effect in effects {
            let samples = synthesize(effect);
            assert!(!samples.is_empty(), "{effect:?} is silent");
            assert!(samples.iter().any(|sample| *sample != 0.0));
            assert!(samples.iter().all(|sample| sample.is_finite()));
            assert!(samples.iter().all(|sample| sample.abs() <= 1.0));
        }
    }

    #[test]
    fn procedural_noise_and_effects_are_deterministic_and_distinct() {
        assert_eq!(
            synthesize(SoundEffect::Breach),
            synthesize(SoundEffect::Breach)
        );
        assert_ne!(
            synthesize(SoundEffect::LinkConnected),
            synthesize(SoundEffect::LinkDisconnected)
        );
        assert!(synthesize(SoundEffect::Victory).len() > synthesize(SoundEffect::Toggle).len());
    }
}
