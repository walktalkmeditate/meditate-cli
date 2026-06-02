pub mod bells;
#[cfg(feature = "audio")]
pub mod output;

use std::sync::Arc;

pub const SAMPLE_RATE: u32 = 44_100;

#[derive(Clone)]
struct Voice {
    samples: Arc<Vec<f32>>,
    pos: usize,
}

/// A minimal mono mixer: master volume, mute, and one-shot voices. Looping
/// soundscape layers, crossfade, and manual ducking are layered on in U7.
pub struct Mixer {
    master: f32,
    muted: bool,
    voices: Vec<Voice>,
}

impl Default for Mixer {
    fn default() -> Mixer {
        Mixer::new()
    }
}

impl Mixer {
    pub fn new() -> Mixer {
        Mixer {
            master: 0.8,
            muted: false,
            voices: Vec::new(),
        }
    }

    pub fn play(&mut self, samples: Arc<Vec<f32>>) {
        if !samples.is_empty() {
            self.voices.push(Voice { samples, pos: 0 });
        }
    }

    pub fn set_master(&mut self, volume: f32) {
        self.master = volume.clamp(0.0, 1.0);
    }

    pub fn master(&self) -> f32 {
        self.master
    }

    pub fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
    }

    pub fn is_muted(&self) -> bool {
        self.muted
    }

    pub fn is_active(&self) -> bool {
        !self.voices.is_empty()
    }

    /// Fill `out` (mono) with the next mixed block, advancing each voice and
    /// dropping any that have finished. Mute zeroes the output without losing
    /// the master level.
    pub fn render(&mut self, out: &mut [f32]) {
        let gain = if self.muted {
            0.0
        } else {
            self.master.max(0.0)
        };
        for slot in out.iter_mut() {
            let mut sample = 0.0;
            for voice in &mut self.voices {
                if let Some(&value) = voice.samples.get(voice.pos) {
                    sample += value;
                    voice.pos += 1;
                }
            }
            *slot = sample * gain;
        }
        self.voices.retain(|voice| voice.pos < voice.samples.len());
    }
}

/// The audio output seam. The real implementation drives a `cpal` stream; the
/// silent one is a safe no-op for headless runs and machines with no device.
pub trait AudioBackend {
    fn bell(&self);
    fn set_master(&self, volume: f32);
    fn set_muted(&self, muted: bool);
}

pub struct SilentBackend;

impl AudioBackend for SilentBackend {
    fn bell(&self) {}
    fn set_master(&self, _volume: f32) {}
    fn set_muted(&self, _muted: bool) {}
}

/// Open the best available audio backend, falling back to silence when the
/// `audio` feature is off or no output device is present.
pub fn open() -> Box<dyn AudioBackend> {
    #[cfg(feature = "audio")]
    {
        if let Some(backend) = output::CpalBackend::try_open() {
            return Box::new(backend);
        }
    }
    Box::new(SilentBackend)
}
