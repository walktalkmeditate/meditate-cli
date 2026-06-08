pub mod cell_gradient;
pub mod mono;
pub mod orb;
pub mod starfield;

use crate::caps::{Capabilities, ColorDepth};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Rgb {
        Rgb { r, g, b }
    }

    pub const BLACK: Rgb = Rgb::new(0, 0, 0);

    pub fn lerp(a: Rgb, b: Rgb, t: f32) -> Rgb {
        let t = t.clamp(0.0, 1.0);
        let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
        Rgb::new(mix(a.r, b.r), mix(a.g, b.g), mix(a.b, b.b))
    }

    /// Perceived brightness in 0.0..1.0, used by the monochrome tier.
    pub fn luma(self) -> f32 {
        (0.299 * self.r as f32 + 0.587 * self.g as f32 + 0.114 * self.b as f32) / 255.0
    }
}

/// A glyph painted into a terminal cell, overriding the half-block fill. `fg` is
/// the glyph's color on the color tier; the mono tier ignores it and emits the
/// glyph as presence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlyphCell {
    pub ch: char,
    pub fg: Rgb,
}

/// A grid of RGB pixels the orb paints into. Two vertical pixels map to one
/// terminal cell via the `▀` upper-half-block trick, so `height` should be even.
/// An optional per-cell glyph layer (one entry per terminal cell, i.e.
/// `width × height/2`) lets the starfield place characters over the half-blocks.
#[derive(Clone, Debug)]
pub struct Surface {
    width: usize,
    height: usize,
    pixels: Vec<Rgb>,
    glyphs: Vec<Option<GlyphCell>>,
}

impl Surface {
    pub fn new(width: usize, height: usize, background: Rgb) -> Surface {
        Surface {
            width,
            height,
            pixels: vec![background; width * height],
            glyphs: vec![None; width * (height / 2)],
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn fill(&mut self, color: Rgb) {
        self.pixels.iter_mut().for_each(|p| *p = color);
    }

    pub fn get(&self, x: usize, y: usize) -> Rgb {
        self.pixels[y * self.width + x]
    }

    pub fn set(&mut self, x: usize, y: usize, color: Rgb) {
        if x < self.width && y < self.height {
            self.pixels[y * self.width + x] = color;
        }
    }

    /// Composite `color` over the existing pixel with the given alpha.
    pub fn blend(&mut self, x: usize, y: usize, color: Rgb, alpha: f32) {
        if x < self.width && y < self.height {
            let under = self.get(x, y);
            self.set(x, y, Rgb::lerp(under, color, alpha));
        }
    }

    /// Number of terminal cells tall (two pixel rows per cell).
    fn cell_rows(&self) -> usize {
        self.height / 2
    }

    /// Place a glyph in a cell (`cell_y` is in cell rows, not pixel rows),
    /// overriding the half-block fill when the surface is encoded.
    pub fn set_glyph(&mut self, x: usize, cell_y: usize, ch: char, fg: Rgb) {
        if x < self.width && cell_y < self.cell_rows() {
            self.glyphs[cell_y * self.width + x] = Some(GlyphCell { ch, fg });
        }
    }

    /// Remove any glyph from a cell, so it renders as a normal half-block.
    pub fn clear_glyph(&mut self, x: usize, cell_y: usize) {
        if x < self.width && cell_y < self.cell_rows() {
            self.glyphs[cell_y * self.width + x] = None;
        }
    }

    /// The glyph in a cell, if any.
    pub fn glyph(&self, x: usize, cell_y: usize) -> Option<GlyphCell> {
        if x < self.width && cell_y < self.cell_rows() {
            self.glyphs[cell_y * self.width + x]
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tier {
    /// Truecolor or 256-color half-block gradient.
    CellGradient,
    /// Shaded blocks with no color (16-color, NO_COLOR, dumb terminals).
    Mono,
}

impl Tier {
    pub fn select(caps: &Capabilities) -> Tier {
        match caps.color {
            ColorDepth::Truecolor | ColorDepth::Ansi256 => Tier::CellGradient,
            ColorDepth::Ansi16 | ColorDepth::None => Tier::Mono,
        }
    }
}

pub trait Renderer {
    /// Encode the surface as a block of ANSI text. The caller positions the
    /// cursor and handles screen clearing; this only renders the orb cells.
    fn encode(&self, surface: &Surface) -> String;
}

/// Build the renderer for a terminal's capabilities. Inline-graphics tiers
/// (Kitty/iTerm2) live in the native CLI; `CellGradient` is the richest tier
/// this pure crate emits, and the one the web build uses.
pub fn renderer_for(caps: &Capabilities) -> Box<dyn Renderer> {
    match Tier::select(caps) {
        Tier::CellGradient => Box::new(cell_gradient::CellGradient::new(caps.color)),
        Tier::Mono => Box::new(mono::Mono),
    }
}

pub(crate) const RESET: &str = "\x1b[0m";

/// Quantize an RGB color to the xterm 256-color palette (6×6×6 cube + grays).
pub(crate) fn to_ansi256(c: Rgb) -> u8 {
    let component = |v: u8| -> u8 {
        if v < 48 {
            0
        } else if v < 115 {
            1
        } else {
            ((v as u16 - 35) / 40) as u8
        }
    };
    let (r, g, b) = (component(c.r), component(c.g), component(c.b));
    16 + 36 * r + 6 * g + b
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::caps::ColorDepth;

    #[test]
    fn glyph_layer_is_noop_when_unset() {
        // AE6: with no glyphs set, both encoders behave as the half-block path.
        let mut s = Surface::new(2, 2, Rgb::new(10, 12, 16));
        s.set(0, 0, Rgb::new(96, 138, 102));
        let cg = cell_gradient::CellGradient::new(ColorDepth::Truecolor).encode(&s);
        assert!(cg.contains('▀'));
        assert!(!cg.contains('✦'));
        let mono = mono::Mono.encode(&s);
        assert!(!mono.contains('\x1b'));
    }

    #[test]
    fn cell_gradient_emits_glyph_in_fg_over_bottom_bg() {
        let mut s = Surface::new(1, 2, Rgb::new(6, 8, 14));
        s.set_glyph(0, 0, '✦', Rgb::new(200, 220, 200));
        let out = cell_gradient::CellGradient::new(ColorDepth::Truecolor).encode(&s);
        assert!(out.contains('✦'));
        assert!(!out.contains('▀'));
        assert!(out.contains("\x1b[38;2;200;220;200m")); // glyph fg
        assert!(out.contains("\x1b[48;2;6;8;14m")); // deep-space bottom bg
    }

    #[test]
    fn transparent_bg_blanks_pure_background_cells() {
        let bg = Rgb::new(10, 12, 16);
        let mut s = Surface::new(2, 2, bg); // cell (1,0) stays pure background
        s.set(0, 0, Rgb::new(96, 138, 102)); // cell (0,0) gets an orb pixel
        let mut cg = cell_gradient::CellGradient::new(ColorDepth::Truecolor);
        cg.set_transparent_bg(Some(bg));
        let out = cg.encode(&s);
        assert!(out.contains("\x1b[49m ")); // pure-bg cell → blank, default bg
        assert!(out.contains('▀')); // the orb cell still renders opaque
                                    // Without transparent mode the same surface has no default-bg blanks.
        let opaque = cell_gradient::CellGradient::new(ColorDepth::Truecolor).encode(&s);
        assert!(!opaque.contains("\x1b[49m"));
    }

    #[test]
    fn mono_emits_glyph_as_presence_without_color() {
        let mut s = Surface::new(1, 2, Rgb::BLACK);
        s.set_glyph(0, 0, '·', Rgb::new(200, 200, 200));
        let out = mono::Mono.encode(&s);
        assert!(out.contains('·'));
        assert!(!out.contains('\x1b'));
    }

    #[test]
    fn set_glyph_ignores_out_of_bounds_and_clear_works() {
        let mut s = Surface::new(2, 2, Rgb::BLACK);
        s.set_glyph(9, 9, '✦', Rgb::BLACK);
        assert_eq!(s.glyph(9, 9), None);
        s.set_glyph(0, 0, '✦', Rgb::BLACK);
        assert!(s.glyph(0, 0).is_some());
        s.clear_glyph(0, 0);
        assert_eq!(s.glyph(0, 0), None);
    }
}
