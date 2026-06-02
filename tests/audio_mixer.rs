use meditate::audio::{Mixer, DUCK_LEVEL};
use std::sync::Arc;

fn render_n(mixer: &mut Mixer, n: usize) {
    let mut block = vec![0.0f32; n];
    mixer.render(&mut block);
}

fn constant(value: f32) -> Arc<Vec<f32>> {
    Arc::new(vec![value])
}

#[test]
fn soundscape_loops_and_fades_in() {
    let mut mixer = Mixer::new();
    mixer.set_sample_rate(2);
    mixer.play_soundscape(constant(0.5));

    render_n(&mut mixer, 32);

    assert!(mixer.has_soundscape());
    assert!(mixer.soundscape_gain().unwrap() > 0.9);
}

#[test]
fn switching_soundscape_crossfades_to_a_single_layer() {
    let mut mixer = Mixer::new();
    mixer.set_sample_rate(2);

    mixer.play_soundscape(constant(0.5));
    render_n(&mut mixer, 20);
    mixer.play_soundscape(constant(0.3));
    assert_eq!(mixer.soundscape_count(), 2);

    render_n(&mut mixer, 40);
    assert_eq!(mixer.soundscape_count(), 1);
    assert!(mixer.soundscape_gain().unwrap() > 0.9);
}

#[test]
fn voice_ducks_the_soundscape_then_restores_it() {
    let mut mixer = Mixer::new();
    mixer.set_sample_rate(2);

    mixer.play_soundscape(constant(0.5));
    render_n(&mut mixer, 20);
    assert!(mixer.soundscape_gain().unwrap() > 0.9);

    mixer.play_voice(Arc::new(vec![1.0; 100]));
    render_n(&mut mixer, 4);
    assert!(mixer.soundscape_gain().unwrap() <= DUCK_LEVEL + 0.05);

    render_n(&mut mixer, 150);
    assert!(!mixer.has_active_voice());
    render_n(&mut mixer, 10);
    assert!(mixer.soundscape_gain().unwrap() > 0.9);
}

#[test]
fn one_shot_bells_still_mix_and_clear() {
    let mut mixer = Mixer::new();
    mixer.set_master(1.0);
    mixer.play(Arc::new(vec![1.0, 0.5]));

    let mut out = [0.0; 4];
    mixer.render(&mut out);
    assert_eq!(out, [1.0, 0.5, 0.0, 0.0]);
    assert!(!mixer.is_active());
}
