pub mod bells;
#[cfg(feature = "audio")]
pub mod output;
pub mod voice;

use std::sync::Arc;

pub const SAMPLE_RATE: u32 = 44_100;
pub const CROSSFADE_SECS: f32 = 4.0;
/// Tail-into-head crossfade length for a seamless soundscape loop (no audible
/// restart at the wrap). Mirrors web/src/audio.ts.
pub const LOOP_CROSSFADE_SECS: f32 = 3.0;
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
        // Fold the tail into the head so the loop has no audible seam at the wrap.
        let xfade = (LOOP_CROSSFADE_SECS * self.sample_rate as f32) as usize;
        let looped = Arc::new(seamless_loop(&samples, xfade));
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
            samples: looped,
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
            .rev()
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

/// Crossfade a mono clip's tail into its head so it loops without a seam. The
/// result is `len - xfade` samples: the first `xfade` blend the head (fading in)
/// with the folded-in tail (fading out), so the wrap from the last sample back
/// to the first continues the original waveform. Mirrors `crossfadeLoopChannel`
/// in web/src/audio.ts.
fn seamless_loop(samples: &[f32], xfade: usize) -> Vec<f32> {
    let xfade = xfade.min(samples.len() / 2);
    if xfade < 1 {
        return samples.to_vec();
    }
    let out_len = samples.len() - xfade;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        if i < xfade {
            let (fade_in, fade_out) = equal_power(i as f32 / xfade as f32);
            out.push(samples[i] * fade_in + samples[i + out_len] * fade_out);
        } else {
            out.push(samples[i]);
        }
    }
    out
}

/// Equal-power crossfade gains (sum of squares ≈ 1) for `t` in 0..1.
fn equal_power(t: f32) -> (f32, f32) {
    let x = t.clamp(0.0, 1.0) * std::f32::consts::FRAC_PI_2;
    (x.sin(), x.cos())
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
    /// Ring a decoded bell sample as a one-shot (no soundscape ducking).
    fn play_bell(&self, samples: Arc<Vec<f32>>);
    fn set_master(&self, volume: f32);
    fn set_muted(&self, muted: bool);
    fn play_soundscape(&self, samples: Arc<Vec<f32>>);
    fn stop_soundscape(&self);
    fn play_voice(&self, samples: Arc<Vec<f32>>);
    /// The output sample rate, so callers decode/resample to match.
    fn sample_rate(&self) -> u32;
}

pub struct SilentBackend;

impl AudioBackend for SilentBackend {
    fn bell(&self) {}
    fn play_bell(&self, _samples: Arc<Vec<f32>>) {}
    fn set_master(&self, _volume: f32) {}
    fn set_muted(&self, _muted: bool) {}
    fn play_soundscape(&self, _samples: Arc<Vec<f32>>) {}
    fn stop_soundscape(&self) {}
    fn play_voice(&self, _samples: Arc<Vec<f32>>) {}
    fn sample_rate(&self) -> u32 {
        SAMPLE_RATE
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seamless_loop_shortens_and_makes_the_wrap_continuous() {
        // A ramp 0..9 whose raw wrap (9 -> 0) is a hard discontinuity.
        let src: Vec<f32> = (0..10).map(|i| i as f32).collect();
        let out = seamless_loop(&src, 2);

        assert_eq!(out.len(), 8); // len - xfade
        assert!((out[2] - 2.0).abs() < 1e-5); // body unchanged past the fade
        assert!((out[7] - 7.0).abs() < 1e-5); // last sample is src[7]
                                              // out[0] folds in the tail (src[len - xfade] = src[8]), so out[7] -> out[0]
                                              // continues 7 -> 8 instead of jumping 9 -> 0.
        assert!((out[0] - 8.0).abs() < 1e-5);
    }

    #[test]
    fn seamless_loop_leaves_a_too_short_clip_alone() {
        assert_eq!(seamless_loop(&[5.0], 4), vec![5.0]);
    }
}
