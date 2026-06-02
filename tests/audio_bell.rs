use meditate::audio::{bells, open, AudioBackend, Mixer, SilentBackend, SAMPLE_RATE};
use std::sync::Arc;

#[test]
fn mixer_plays_a_one_shot_then_goes_idle() {
    let mut mixer = Mixer::new();
    mixer.set_master(1.0);
    mixer.play(Arc::new(vec![1.0, 0.5]));
    assert!(mixer.is_active());

    let mut out = [0.0; 4];
    mixer.render(&mut out);
    assert_eq!(out, [1.0, 0.5, 0.0, 0.0]);
    assert!(!mixer.is_active());
}

#[test]
fn master_volume_scales_output() {
    let mut mixer = Mixer::new();
    mixer.set_master(0.5);
    mixer.play(Arc::new(vec![1.0]));
    let mut out = [0.0; 1];
    mixer.render(&mut out);
    assert!((out[0] - 0.5).abs() < 1e-6);
}

#[test]
fn mute_silences_without_losing_master_level() {
    let mut mixer = Mixer::new();
    mixer.set_master(1.0);
    mixer.set_muted(true);
    mixer.play(Arc::new(vec![1.0, 1.0]));

    let mut out = [0.0; 2];
    mixer.render(&mut out);
    assert_eq!(out, [0.0, 0.0]);
    assert!((mixer.master() - 1.0).abs() < 1e-6);
}

#[test]
fn synth_bell_starts_quiet_decays_and_stays_in_range() {
    let bell = bells::synth_bell(SAMPLE_RATE);
    assert!(bell.len() > SAMPLE_RATE as usize);
    assert!(bell.iter().all(|s| s.abs() <= 1.0));
    assert!(bell[0].abs() < 1e-3);

    let quarter = bell.len() / 4;
    let rms =
        |slice: &[f32]| (slice.iter().map(|s| s * s).sum::<f32>() / slice.len() as f32).sqrt();
    assert!(rms(&bell[..quarter]) > rms(&bell[bell.len() - quarter..]));
}

#[test]
fn silent_backend_and_open_never_panic_headless() {
    let silent = SilentBackend;
    silent.bell();
    silent.set_master(0.5);
    silent.set_muted(true);

    let backend = open();
    backend.bell();
    backend.set_master(0.3);
    backend.set_muted(false);
}
