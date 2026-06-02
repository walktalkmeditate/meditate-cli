use std::f32::consts::PI;

/// Synthesize a soft bell tone: a struck C5 with a few inharmonic partials and
/// an exponential decay. Generating it procedurally means the built-in bell
/// needs no bundled audio asset and no decoder.
pub fn synth_bell(sample_rate: u32) -> Vec<f32> {
    let duration_secs = 1.6;
    let count = (sample_rate as f32 * duration_secs) as usize;
    let base = 523.25;
    let partials = [(1.0, 1.0), (2.01, 0.5), (2.99, 0.25), (4.2, 0.12)];

    (0..count)
        .map(|i| {
            let t = i as f32 / sample_rate as f32;
            let envelope = (-3.0 * t).exp();
            let tone: f32 = partials
                .iter()
                .map(|(mult, amp)| amp * (2.0 * PI * base * mult * t).sin())
                .sum();
            0.18 * envelope * tone
        })
        .collect()
}
