use super::{bells, AudioBackend, Mixer};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};

/// Real audio output: a `cpal` stream that pulls mixed mono samples from a
/// shared `Mixer` and fans them across the device's channels.
pub struct CpalBackend {
    mixer: Arc<Mutex<Mixer>>,
    bell: Arc<Vec<f32>>,
    _stream: cpal::Stream,
}

impl CpalBackend {
    pub fn try_open() -> Option<CpalBackend> {
        let host = cpal::default_host();
        let device = host.default_output_device()?;
        let config = device.default_output_config().ok()?;
        let sample_rate = config.sample_rate().0;
        let channels = config.channels() as usize;

        let mixer = Arc::new(Mutex::new(Mixer::new()));
        if let Ok(mut guard) = mixer.lock() {
            guard.set_sample_rate(sample_rate);
        }
        let stream_mixer = Arc::clone(&mixer);

        let stream = device
            .build_output_stream(
                &config.into(),
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    let frames = data.len() / channels.max(1);
                    let mut block = vec![0.0f32; frames];
                    if let Ok(mut mixer) = stream_mixer.lock() {
                        mixer.render(&mut block);
                    }
                    for (frame, sample) in data.chunks_mut(channels.max(1)).zip(block) {
                        for out in frame.iter_mut() {
                            *out = sample;
                        }
                    }
                },
                |_err| {},
                None,
            )
            .ok()?;
        stream.play().ok()?;

        Some(CpalBackend {
            mixer,
            bell: Arc::new(bells::synth_bell(sample_rate)),
            _stream: stream,
        })
    }
}

impl AudioBackend for CpalBackend {
    fn bell(&self) {
        if let Ok(mut mixer) = self.mixer.lock() {
            mixer.play(Arc::clone(&self.bell));
        }
    }

    fn set_master(&self, volume: f32) {
        if let Ok(mut mixer) = self.mixer.lock() {
            mixer.set_master(volume);
        }
    }

    fn set_muted(&self, muted: bool) {
        if let Ok(mut mixer) = self.mixer.lock() {
            mixer.set_muted(muted);
        }
    }
}
