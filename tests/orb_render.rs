use meditate::breath::{Phase, PhaseState};
use meditate::palette::{palette, Season, TimeOfDay};
use meditate::render::orb::{ease_in_out, glow_for, paint, scale_for, OrbScene};
use meditate::render::{Rgb, Surface};

fn phase(phase: Phase, progress: f32) -> PhaseState {
    PhaseState {
        phase,
        progress,
        breath_count: 0,
    }
}

fn assert_close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < 1e-3,
        "expected {expected}, got {actual}"
    );
}

fn scene(scale: f32) -> OrbScene {
    OrbScene {
        scale,
        glow: 0.0,
        ripples: vec![],
        milestone_flash: 0.0,
        palette: palette(Season::Spring, TimeOfDay::Day),
    }
}

fn non_background_pixels(surface: &Surface, background: Rgb) -> usize {
    let mut count = 0;
    for y in 0..surface.height() {
        for x in 0..surface.width() {
            if surface.get(x, y) != background {
                count += 1;
            }
        }
    }
    count
}

#[test]
fn easing_is_symmetric_at_the_ends_and_midpoint() {
    assert_close(ease_in_out(0.0), 0.0);
    assert_close(ease_in_out(1.0), 1.0);
    assert_close(ease_in_out(0.5), 0.5);
}

#[test]
fn scale_grows_on_inhale_and_shrinks_on_exhale() {
    assert_close(scale_for(phase(Phase::Inhale, 0.0)), 0.45);
    assert_close(scale_for(phase(Phase::Inhale, 1.0)), 1.0);
    assert_close(scale_for(phase(Phase::Exhale, 0.0)), 1.0);
    assert_close(scale_for(phase(Phase::Exhale, 1.0)), 0.45);
    assert_close(scale_for(phase(Phase::HoldIn, 0.5)), 1.0);
    assert_close(scale_for(phase(Phase::HoldOut, 0.5)), 0.45);
    assert_close(scale_for(phase(Phase::Still, 0.0)), 0.7);
}

#[test]
fn glow_is_strongest_while_holding() {
    assert_close(glow_for(phase(Phase::HoldIn, 0.5)), 1.0);
    assert_close(glow_for(phase(Phase::HoldOut, 0.5)), 0.6);
    assert_close(glow_for(phase(Phase::Inhale, 0.5)), 0.0);
}

#[test]
fn orb_paints_over_a_clean_background() {
    let pal = palette(Season::Spring, TimeOfDay::Day);
    let mut surface = Surface::new(20, 20, Rgb::BLACK);
    paint(&mut surface, &scene(1.0));

    assert_eq!(surface.get(0, 0), pal.background);
    assert_ne!(surface.get(10, 10), pal.background);
}

#[test]
fn larger_scale_fills_more_pixels() {
    let pal = palette(Season::Spring, TimeOfDay::Day);

    let mut small = Surface::new(24, 24, Rgb::BLACK);
    paint(&mut small, &scene(0.45));

    let mut large = Surface::new(24, 24, Rgb::BLACK);
    paint(&mut large, &scene(1.0));

    assert!(
        non_background_pixels(&large, pal.background)
            > non_background_pixels(&small, pal.background)
    );
}

#[test]
fn milestone_flash_brightens_the_edge() {
    let mut plain = Surface::new(24, 24, Rgb::BLACK);
    paint(&mut plain, &scene(0.7));

    let mut flashed = Surface::new(24, 24, Rgb::BLACK);
    let mut flash_scene = scene(0.7);
    flash_scene.milestone_flash = 1.0;
    paint(&mut flashed, &flash_scene);

    // The flash blends ripple color into the edge ring (~radius 7.7 from the
    // center at 12,12), so an edge pixel differs from the un-flashed frame.
    assert_ne!(plain.get(12, 4), flashed.get(12, 4));
}
