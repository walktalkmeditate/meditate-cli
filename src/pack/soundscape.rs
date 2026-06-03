use std::path::Path;

/// Decode a cached audio file into mono f32 samples, resampled to `target_rate`
/// (the device's rate, so playback isn't pitch-shifted). Returns `None` when
/// decoding fails or the `audio` feature is off — the session then falls back to
/// the missing-pack hint, so an absent decoder never breaks the breathing screen.
#[cfg(feature = "audio")]
pub fn load_samples(path: &Path, target_rate: u32) -> Option<Vec<f32>> {
    decode::decode_mono(path, target_rate)
}

#[cfg(not(feature = "audio"))]
pub fn load_samples(_path: &Path, _target_rate: u32) -> Option<Vec<f32>> {
    None
}

#[cfg(feature = "audio")]
mod decode {
    use std::path::Path;
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::DecoderOptions;
    use symphonia::core::errors::Error;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    pub fn decode_mono(path: &Path, target_rate: u32) -> Option<Vec<f32>> {
        let file = std::fs::File::open(path).ok()?;
        let stream = MediaSourceStream::new(Box::new(file), Default::default());

        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }

        let probed = symphonia::default::get_probe()
            .format(
                &hint,
                stream,
                &FormatOptions::default(),
                &MetadataOptions::default(),
            )
            .ok()?;
        let mut format = probed.format;
        let track = format.default_track()?.clone();
        let track_id = track.id;
        let src_rate = track.codec_params.sample_rate?;
        let mut decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &DecoderOptions::default())
            .ok()?;

        let mut mono: Vec<f32> = Vec::new();
        loop {
            let packet = match format.next_packet() {
                Ok(packet) => packet,
                Err(_) => break,
            };
            if packet.track_id() != track_id {
                continue;
            }
            let decoded = match decoder.decode(&packet) {
                Ok(decoded) => decoded,
                Err(Error::DecodeError(_)) => continue,
                Err(_) => break,
            };
            let spec = *decoded.spec();
            let channels = spec.channels.count().max(1);
            let capacity = decoded.capacity() as u64;
            let mut buffer = SampleBuffer::<f32>::new(capacity, spec);
            buffer.copy_interleaved_ref(decoded);
            for frame in buffer.samples().chunks(channels) {
                mono.push(frame.iter().sum::<f32>() / channels as f32);
            }
        }

        if mono.is_empty() {
            return None;
        }
        Some(resample_linear(&mono, src_rate, target_rate))
    }

    fn resample_linear(input: &[f32], from: u32, to: u32) -> Vec<f32> {
        if from == to || input.len() < 2 {
            return input.to_vec();
        }
        let ratio = f64::from(to) / f64::from(from);
        let out_len = (input.len() as f64 * ratio) as usize;
        let mut out = Vec::with_capacity(out_len);
        for i in 0..out_len {
            let pos = i as f64 / ratio;
            let idx = pos.floor() as usize;
            let frac = (pos - idx as f64) as f32;
            let a = input[idx.min(input.len() - 1)];
            let b = *input.get(idx + 1).unwrap_or(&a);
            out.push(a + (b - a) * frac);
        }
        out
    }
}

#[cfg(all(test, feature = "audio"))]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_wav(path: &Path, samples: &[i16], rate: u32) {
        let data_len = (samples.len() * 2) as u32;
        let mut f = std::fs::File::create(path).unwrap();
        let mut put = |bytes: &[u8]| f.write_all(bytes).unwrap();
        put(b"RIFF");
        put(&(36 + data_len).to_le_bytes());
        put(b"WAVE");
        put(b"fmt ");
        put(&16u32.to_le_bytes());
        put(&1u16.to_le_bytes()); // PCM
        put(&1u16.to_le_bytes()); // mono
        put(&rate.to_le_bytes());
        put(&(rate * 2).to_le_bytes()); // byte rate
        put(&2u16.to_le_bytes()); // block align
        put(&16u16.to_le_bytes()); // bits per sample
        put(b"data");
        put(&data_len.to_le_bytes());
        for s in samples {
            put(&s.to_le_bytes());
        }
    }

    #[test]
    fn decodes_wav_to_mono_and_resamples() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tone.wav");
        let samples: Vec<i16> = (0..800)
            .map(|i| ((i as f32 * 0.05).sin() * 10_000.0) as i16)
            .collect();
        write_wav(&path, &samples, 8000);

        let native = load_samples(&path, 8000).unwrap();
        assert_eq!(native.len(), 800);
        assert!(native.iter().all(|s| s.abs() <= 1.0));

        let upsampled = load_samples(&path, 16_000).unwrap();
        assert_eq!(upsampled.len(), 1600);
    }
}
