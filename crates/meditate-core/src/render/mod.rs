pub mod cell_gradient;
pub mod mono;
pub mod orb;

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

/// A grid of RGB pixels the orb paints into. Two vertical pixels map to one
/// terminal cell via the `▀` upper-half-block trick, so `height` should be even.
#[derive(Clone, Debug)]
pub struct Surface {
    width: usize,
    height: usize,
    pixels: Vec<Rgb>,
}

impl Surface {
    pub fn new(width: usize, height: usize, background: Rgb) -> Surface {
        Surface {
            width,
            height,
            pixels: vec![background; width * height],
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
