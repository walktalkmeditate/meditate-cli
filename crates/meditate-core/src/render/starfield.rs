//! The constellation starfield: a pure, deterministic, resolution-independent
//! model. Star positions live in a normalized 0..1 space generated once from a
//! seed, so a resize reflows the field (the same stars remap to new cells)
//! rather than reshuffling it. The model emits per-star brightness; mapping that
//! to color or to mono presence is the render layer's job.

use crate::breath::Phase;
use crate::render::{Rgb, Surface};

/// Fixed pool of stars. Density emerges from projecting these onto a surface;
/// area-scaled density is a deferred tuning concern.
const STAR_COUNT: usize = 160;
const NEAR_FRACTION: f32 = 0.3;
/// Brightness bands per tier (disjoint, so near stars always read brighter).
const NEAR_BRIGHT: (f32, f32) = (0.55, 0.95);
const FAR_BRIGHT: (f32, f32) = (0.18, 0.45);
const NEAR_GLYPHS: [char; 3] = ['✦', '✧', '∗'];
const FAR_GLYPHS: [char; 3] = ['·', '⋆', '∙'];

/// How much extra brightness and how far out (in cells) the near tier blooms at
/// the exhale peak. Tunable.
const BLOOM_GAIN: f32 = 0.4;
const BLOOM_OFFSET: f32 = 1.5;

/// Soft moss-white starlight; dim stars lerp from the background toward this.
const STAR_COLOR: Rgb = Rgb::new(196, 214, 200);

/// A star in normalized space, fixed for the life of the field.
struct NormStar {
    nx: f32,
    ny: f32,
    glyph: char,
    brightness: f32,
    near: bool,
}

/// A star projected onto a concrete surface, in cell coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Star {
    pub x: usize,
    pub cell_y: usize,
    pub glyph: char,
    /// Base brightness 0..1, before any breath bloom is applied.
    pub brightness: f32,
    /// Near-tier stars bloom with the breath; far-tier stars stay static.
    pub near: bool,
}

/// The breath bloom applied to near-tier stars: extra brightness and a small
/// outward radial offset, peaking at the exhale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bloom {
    pub gain: f32,
    pub offset: f32,
}

pub struct Starfield {
    stars: Vec<NormStar>,
}

impl Starfield {
    /// Generate a field deterministically from a seed.
    pub fn new(seed: u64) -> Starfield {
        let mut state = seed ^ 0x5DEE_CE66_D000_0000;
        let mut stars = Vec::with_capacity(STAR_COUNT);
        for _ in 0..STAR_COUNT {
            let nx = unit(&mut state);
            let ny = unit(&mut state);
            let near = unit(&mut state) < NEAR_FRACTION;
            let (lo, hi) = if near { NEAR_BRIGHT } else { FAR_BRIGHT };
            let glyphs = if near { NEAR_GLYPHS } else { FAR_GLYPHS };
            let idx = ((unit(&mut state) * glyphs.len() as f32) as usize).min(glyphs.len() - 1);
            let brightness = lo + unit(&mut state) * (hi - lo);
            stars.push(NormStar {
                nx,
                ny,
                glyph: glyphs[idx],
                brightness,
                near,
            });
        }
        Starfield { stars }
    }

    /// Project the field onto a `width × height` pixel surface (two pixels per
    /// cell row), dropping stars within `clearing_radius` pixels of the centered
    /// orb so the moss glow stays clear.
    pub fn cells(&self, width: usize, height: usize, clearing_radius: f32) -> Vec<Star> {
        let cell_rows = height / 2;
        if width == 0 || cell_rows == 0 {
            return Vec::new();
        }
        let ocx = width as f32 / 2.0;
        let ocy = height as f32 / 2.0;
        let mut out = Vec::new();
        for s in &self.stars {
            let x = ((s.nx * width as f32) as usize).min(width - 1);
            let cell_y = ((s.ny * cell_rows as f32) as usize).min(cell_rows - 1);
            // Cell center in pixel space (the cell spans pixel rows cy*2, cy*2+1).
            let px = x as f32 + 0.5;
            let py = (cell_y * 2) as f32 + 1.0;
            let dist = ((px - ocx).powi(2) + (py - ocy).powi(2)).sqrt();
            if dist < clearing_radius {
                continue;
            }
            out.push(Star {
                x,
                cell_y,
                glyph: s.glyph,
                brightness: s.brightness,
                near: s.near,
            });
        }
        out
    }
}

/// The bloom for a breath phase. Exhale eases the near tier out and bright;
/// hold-out holds it at peak; inhale settles it; hold-in and still rest at zero.
/// Read directly from the breath `PhaseState` — no scale-trajectory guessing.
pub fn bloom(phase: Phase, progress: f32) -> Bloom {
    let amount = match phase {
        Phase::Exhale => progress.clamp(0.0, 1.0),
        Phase::HoldOut => 1.0,
        Phase::Inhale => 1.0 - progress.clamp(0.0, 1.0),
        Phase::HoldIn | Phase::Still => 0.0,
    };
    Bloom {
        gain: amount * BLOOM_GAIN,
        offset: amount * BLOOM_OFFSET,
    }
}

/// Write the projected stars into the surface's glyph layer. The orb wins: a
/// star is dropped wherever the orb has already painted a non-background pixel
/// (glyph-erase on collision), so the moss glow is never pierced. Near stars
/// take the bloom's brightness gain and ease outward by its offset.
pub fn paint(surface: &mut Surface, stars: &[Star], bloom: Bloom, background: Rgb) {
    let width = surface.width();
    let cell_rows = surface.height() / 2;
    if width == 0 || cell_rows == 0 {
        return;
    }
    let ccx = width as f32 / 2.0;
    let ccy = cell_rows as f32 / 2.0;
    for star in stars {
        let (mut x, mut cy) = (star.x, star.cell_y);
        let mut brightness = star.brightness;
        if star.near {
            brightness = (brightness + bloom.gain).min(1.0);
            if bloom.offset > 0.0 {
                let dx = x as f32 + 0.5 - ccx;
                let dy = cy as f32 + 0.5 - ccy;
                let len = (dx * dx + dy * dy).sqrt().max(0.001);
                let ox = (x as f32 + 0.5 + dx / len * bloom.offset).floor();
                let oy = (cy as f32 + 0.5 + dy / len * bloom.offset).floor();
                if ox >= 0.0 && oy >= 0.0 && (ox as usize) < width && (oy as usize) < cell_rows {
                    x = ox as usize;
                    cy = oy as usize;
                }
            }
        }
        // Orb wins: skip any cell the orb has already painted into.
        if surface.get(x, cy * 2) != background || surface.get(x, cy * 2 + 1) != background {
            continue;
        }
        surface.set_glyph(
            x,
            cy,
            star.glyph,
            Rgb::lerp(background, STAR_COLOR, brightness),
        );
    }
}

/// SplitMix64 → a deterministic float in `[0, 1)`. Keeps the field reproducible
/// without an RNG dependency (the core stays dependency-free).
fn unit(state: &mut u64) -> f32 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    // Top 24 bits → [0, 1).
    (z >> 40) as f32 / (1u64 << 24) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_is_deterministic() {
        let a = Starfield::new(7).cells(80, 24, 5.0);
        let b = Starfield::new(7).cells(80, 24, 5.0);
        assert_eq!(a, b);
        assert!(!a.is_empty());
    }

    #[test]
    fn clearing_excludes_stars_near_the_orb() {
        let r = 12.0;
        let stars = Starfield::new(3).cells(80, 24, r);
        let (ocx, ocy) = (40.0_f32, 12.0_f32);
        for s in &stars {
            let px = s.x as f32 + 0.5;
            let py = (s.cell_y * 2) as f32 + 1.0;
            let dist = ((px - ocx).powi(2) + (py - ocy).powi(2)).sqrt();
            assert!(
                dist >= r,
                "star {:?} is inside the clearing",
                (s.x, s.cell_y)
            );
        }
    }

    #[test]
    fn resize_reflows_rather_than_reshuffles() {
        // With no clearing, both sizes keep every star in the same order with the
        // same identity (glyph/brightness/tier) — only the cell mapping changes.
        let field = Starfield::new(11);
        let small = field.cells(80, 24, 0.0);
        let large = field.cells(160, 48, 0.0);
        assert_eq!(small.len(), large.len());
        for (s, l) in small.iter().zip(large.iter()) {
            assert_eq!(s.glyph, l.glyph);
            assert_eq!(s.brightness, l.brightness);
            assert_eq!(s.near, l.near);
        }
    }

    #[test]
    fn tiers_separate_near_bright_from_far_dim() {
        let stars = Starfield::new(5).cells(200, 60, 0.0);
        for s in &stars {
            if s.near {
                assert!(s.brightness >= NEAR_BRIGHT.0);
            } else {
                assert!(s.brightness <= FAR_BRIGHT.1);
            }
        }
        assert!(NEAR_BRIGHT.0 > FAR_BRIGHT.1);
    }

    #[test]
    fn degenerate_sizes_produce_no_stars_and_no_panic() {
        assert!(Starfield::new(1).cells(0, 24, 5.0).is_empty());
        assert!(Starfield::new(1).cells(80, 1, 5.0).is_empty());
        assert!(Starfield::new(1).cells(80, 0, 5.0).is_empty());
    }

    #[test]
    fn bloom_peaks_on_exhale_and_settles_on_inhale() {
        assert!(bloom(Phase::Exhale, 1.0).gain > 0.0);
        assert!(bloom(Phase::Exhale, 1.0).offset > 0.0);
        assert_eq!(bloom(Phase::HoldOut, 0.5).gain, BLOOM_GAIN);
        assert_eq!(bloom(Phase::Inhale, 1.0).gain, 0.0);
        assert_eq!(bloom(Phase::HoldIn, 0.5).gain, 0.0);
        assert_eq!(bloom(Phase::Still, 0.5).gain, 0.0);
        assert!(bloom(Phase::Exhale, 1.0).gain <= BLOOM_GAIN);
        assert!(bloom(Phase::Exhale, 1.0).offset <= BLOOM_OFFSET);
    }

    #[test]
    fn paint_drops_stars_on_orb_cells_and_places_them_elsewhere() {
        let bg = Rgb::new(6, 8, 14);
        let mut surface = Surface::new(4, 4, bg); // 4 cols × 2 cell rows
        surface.set(1, 0, Rgb::new(96, 138, 102)); // orb pixel in cell (1, 0)
        let stars = vec![
            Star {
                x: 1,
                cell_y: 0,
                glyph: '✦',
                brightness: 0.9,
                near: false,
            },
            Star {
                x: 3,
                cell_y: 1,
                glyph: '·',
                brightness: 0.3,
                near: false,
            },
        ];
        paint(
            &mut surface,
            &stars,
            Bloom {
                gain: 0.0,
                offset: 0.0,
            },
            bg,
        );
        assert_eq!(surface.glyph(1, 0), None); // orb wins
        assert!(surface.glyph(3, 1).is_some()); // clear cell gets the star
    }

    #[test]
    fn mono_renders_field_as_glyphs_without_color() {
        use crate::render::mono::Mono;
        use crate::render::Renderer;
        let bg = Rgb::new(6, 8, 14);
        let field = Starfield::new(7);
        let mut surface = Surface::new(80, 24, bg);
        let stars = field.cells(80, 24, 0.0);
        paint(
            &mut surface,
            &stars,
            Bloom {
                gain: 0.0,
                offset: 0.0,
            },
            bg,
        );
        let out = Mono.encode(&surface);
        assert!(!out.contains('\x1b')); // AE2: no color codes on the mono tier
        assert!(stars.iter().any(|s| out.contains(s.glyph))); // depth via glyph density
    }
}
