pub mod bells;
#[cfg(feature = "audio")]
pub mod output;

use std::sync::Arc;

pub const SAMPLE_RATE: u32 = 44_100;
pub const CROSSFADE_SECS: f32 = 4.0;
pub const DUCK_LEVEL: f32 = 0.35;
pub const DUCK_FADE_SECS: f32 = 0.5;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Slot {
    Soundscape,
    Voice,
}

#[derive(Clone)]
struct Voice {
    samples: Arc<Vec<f32>>,
    pos: usize,
}

/// A gain-enveloped, optionally-looping layer. `gain` carries soundscape fades
/// and crossfades; `duck` is the separate multiplier voices dip it with.
#[derive(Clone)]
struct Layer {
    samples: Arc<Vec<f32>>,
    pos: usize,
    slot: Slot,
    looping: bool,
    gain: f32,
    gain_target: f32,
    gain_ramp: f32,
    duck: f32,
    duck_target: f32,
    duck_ramp: f32,
    remove_at_zero: bool,
}

impl Layer {
    fn step(&mut self) {
        self.gain = approach(self.gain, self.gain_target, self.gain_ramp);
        self.duck = approach(self.duck, self.duck_target, self.duck_ramp);
    }

    fn next_sample(&mut self) -> Option<f32> {
        let value = *self.samples.get(self.pos)?;
        self.pos += 1;
        if self.looping && self.pos >= self.samples.len() {
            self.pos = 0;
        }
        Some(value * self.gain * self.duck)
    }

    fn finished(&self) -> bool {
        (!self.looping && self.pos >= self.samples.len())
            || (self.remove_at_zero && self.gain <= 0.0005)
    }
}

/// The mixer: one-shot voices (bells), plus looping/ducking layers for
/// soundscapes and voice guides. Output is mono; the backend fans it to the
/// device's channels.
pub struct Mixer {
    sample_rate: u32,
    master: f32,
    muted: bool,
    voices: Vec<Voice>,
    layers: Vec<Layer>,
}

impl Default for Mixer {
    fn default() -> Mixer {
        Mixer::new()
    }
}

impl Mixer {
    pub fn new() -> Mixer {
        Mixer {
            sample_rate: SAMPLE_RATE,
            master: 0.8,
            muted: false,
            voices: Vec::new(),
            layers: Vec::new(),
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: u32) {
        self.sample_rate = sample_rate.max(1);
    }

    pub fn play(&mut self, samples: Arc<Vec<f32>>) {
        if !samples.is_empty() {
            self.voices.push(Voice { samples, pos: 0 });
        }
    }

    /// Crossfade to a new soundscape: existing soundscapes fade out and are
    /// removed while the new one fades in.
    pub fn play_soundscape(&mut self, samples: Arc<Vec<f32>>) {
        if samples.is_empty() {
            return;
        }
        let ramp = ramp_for(CROSSFADE_SECS, self.sample_rate);
        for layer in self
            .layers
            .iter_mut()
            .filter(|l| l.slot == Slot::Soundscape)
        {
            layer.gain_target = 0.0;
            layer.gain_ramp = ramp;
            layer.remove_at_zero = true;
        }
        let ducked = if self.has_active_voice() {
            DUCK_LEVEL
        } else {
            1.0
        };
        self.layers.push(Layer {
            samples,
            pos: 0,
            slot: Slot::Soundscape,
            looping: true,
            gain: 0.0,
            gain_target: 1.0,
            gain_ramp: ramp,
            duck: ducked,
            duck_target: ducked,
            duck_ramp: ramp_for(DUCK_FADE_SECS, self.sample_rate),
            remove_at_zero: false,
        });
    }

    pub fn stop_soundscape(&mut self) {
        let ramp = ramp_for(CROSSFADE_SECS, self.sample_rate);
        for layer in self
            .layers
            .iter_mut()
            .filter(|l| l.slot == Slot::Soundscape)
        {
            layer.gain_target = 0.0;
            layer.gain_ramp = ramp;
            layer.remove_at_zero = true;
        }
    }

    /// Play a one-shot voice prompt, ducking the soundscape beneath it.
    pub fn play_voice(&mut self, samples: Arc<Vec<f32>>) {
        if samples.is_empty() {
            return;
        }
        let duck_ramp = ramp_for(DUCK_FADE_SECS, self.sample_rate);
        for layer in self
            .layers
            .iter_mut()
            .filter(|l| l.slot == Slot::Soundscape)
        {
            layer.duck_target = DUCK_LEVEL;
            layer.duck_ramp = duck_ramp;
        }
        self.layers.push(Layer {
            samples,
            pos: 0,
            slot: Slot::Voice,
            looping: false,
            gain: 1.0,
            gain_target: 1.0,
            gain_ramp: 0.0,
            duck: 1.0,
            duck_target: 1.0,
            duck_ramp: 0.0,
            remove_at_zero: false,
        });
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
        !self.voices.is_empty() || !self.layers.is_empty()
    }

    pub fn has_soundscape(&self) -> bool {
        self.layers.iter().any(|l| l.slot == Slot::Soundscape)
    }

    pub fn soundscape_count(&self) -> usize {
        self.layers
            .iter()
            .filter(|l| l.slot == Slot::Soundscape)
            .count()
    }

    pub fn has_active_voice(&self) -> bool {
        self.layers
            .iter()
            .any(|l| l.slot == Slot::Voice && l.pos < l.samples.len())
    }

    /// Effective gain (fade × duck) of the leading soundscape layer, exposed for
    /// the HUD and for tests.
    pub fn soundscape_gain(&self) -> Option<f32> {
        self.layers
            .iter()
            .find(|l| l.slot == Slot::Soundscape)
            .map(|l| l.gain * l.duck)
    }

    pub fn render(&mut self, out: &mut [f32]) {
        let gain = if self.muted { 0.0 } else { self.master };
        for slot in out.iter_mut() {
            let mut sample = 0.0;
            for voice in &mut self.voices {
                if let Some(&value) = voice.samples.get(voice.pos) {
                    sample += value;
                    voice.pos += 1;
                }
            }
            for layer in &mut self.layers {
                layer.step();
                if let Some(value) = layer.next_sample() {
                    sample += value;
                }
            }
            *slot = sample * gain;
        }

        self.voices.retain(|voice| voice.pos < voice.samples.len());
        self.layers.retain(|layer| !layer.finished());

        if !self.has_active_voice() {
            let ramp = ramp_for(DUCK_FADE_SECS, self.sample_rate);
            for layer in self
                .layers
                .iter_mut()
                .filter(|l| l.slot == Slot::Soundscape)
            {
                layer.duck_target = 1.0;
                if layer.duck_ramp == 0.0 {
                    layer.duck_ramp = ramp;
                }
            }
        }
    }
}

fn approach(current: f32, target: f32, ramp: f32) -> f32 {
    if ramp <= 0.0 || (target - current).abs() <= ramp {
        target
    } else if target > current {
        current + ramp
    } else {
        current - ramp
    }
}

/// Per-sample gain step that moves a 0..1 envelope across `secs` seconds.
fn ramp_for(secs: f32, sample_rate: u32) -> f32 {
    if secs <= 0.0 {
        1.0
    } else {
        1.0 / (secs * sample_rate as f32)
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
