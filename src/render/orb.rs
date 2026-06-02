use crate::breath::{Phase, PhaseState};
use crate::palette::Palette;
use crate::render::{Rgb, Surface};

pub const MIN_SCALE: f32 = 0.45;
pub const MAX_SCALE: f32 = 1.0;
pub const STILL_SCALE: f32 = 0.7;

/// Smoothstep easing, matching the felt curve of the iOS `.easeInOut` orb.
pub fn ease_in_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Orb scale (0.45..1.0) for a breath phase: it grows on inhale, holds full
/// during hold-in, shrinks on exhale, and rests small during hold-out.
pub fn scale_for(state: PhaseState) -> f32 {
    match state.phase {
        Phase::Inhale => lerp(MIN_SCALE, MAX_SCALE, ease_in_out(state.progress)),
        Phase::HoldIn => MAX_SCALE,
        Phase::Exhale => lerp(MAX_SCALE, MIN_SCALE, ease_in_out(state.progress)),
        Phase::HoldOut => MIN_SCALE,
        Phase::Still => STILL_SCALE,
    }
}

/// Inner-glow intensity, brightest while holding a full breath.
pub fn glow_for(state: PhaseState) -> f32 {
    match state.phase {
        Phase::HoldIn => 1.0,
        Phase::HoldOut => 0.6,
        _ => 0.0,
    }
}

#[derive(Clone, Debug)]
pub struct OrbScene {
    pub scale: f32,
    pub glow: f32,
    /// Each ripple's life from 0.0 (just emitted) to 1.0 (faded out).
    pub ripples: Vec<f32>,
    pub milestone_flash: f32,
    pub palette: Palette,
}

/// Paint the orb, ripples, glow, and milestone flash into the surface. Pure: it
/// only writes pixels, leaving cursor handling and the on-screen draw to U5.
pub fn paint(surface: &mut Surface, scene: &OrbScene) {
    surface.fill(scene.palette.background);
    let width = surface.width();
    let height = surface.height();
    if width == 0 || height == 0 {
        return;
    }

    let cx = width as f32 / 2.0;
    let cy = height as f32 / 2.0;
    let base = (width.min(height) as f32 / 2.0) * 0.92;
    let radius = (base * scene.scale).max(1.0);

    for y in 0..height {
        for x in 0..width {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            let dist = (dx * dx + dy * dy).sqrt();

            if dist <= radius {
                let t = dist / radius;
                let body = Rgb::lerp(scene.palette.core, scene.palette.edge, t);
                surface.blend(x, y, body, 1.0 - t * 0.2);
                if scene.glow > 0.0 {
                    let inner = 1.0 - (dist / (radius * 0.5)).min(1.0);
                    surface.blend(x, y, scene.palette.core, inner * scene.glow * 0.35);
                }
            }

            for &life in &scene.ripples {
                let ring_radius = lerp(base * 0.4, base * 1.25, life);
                let edge = (dist - ring_radius).abs();
                if edge < 1.0 {
                    surface.blend(
                        x,
                        y,
                        scene.palette.ripple,
                        (1.0 - life) * 0.3 * (1.0 - edge),
                    );
                }
            }

            if scene.milestone_flash > 0.0 {
                let edge = (dist - radius).abs();
                if edge < 1.5 {
                    surface.blend(x, y, scene.palette.ripple, scene.milestone_flash * 0.5);
                }
            }
        }
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
