use super::{to_ansi256, Renderer, Rgb, Surface, RESET};
use crate::caps::ColorDepth;

/// Renders the surface as truecolor (or 256-color) half-block cells: each cell
/// is `▀`, with the foreground painting the top pixel and the background the
/// bottom pixel, so one character row shows two pixel rows.
pub struct CellGradient {
    color: ColorDepth,
    quantize: u8,
}

impl CellGradient {
    pub fn new(color: ColorDepth) -> CellGradient {
        CellGradient { color, quantize: 1 }
    }

    /// Snap truecolor channels to a `step`, shrinking the number of distinct
    /// fg/bg pairs in a smooth gradient. The native CLI uses `new` (step 1, no
    /// quantization); the web uses this to relieve xterm's WebGL glyph-atlas,
    /// which overflows on a full-screen, per-cell-unique truecolor gradient.
    pub fn quantized(color: ColorDepth, step: u8) -> CellGradient {
        CellGradient {
            color,
            quantize: step.max(1),
        }
    }

    /// Round a channel to the nearest multiple of the quantization step.
    fn q(&self, v: u8) -> u8 {
        let step = self.quantize as u16;
        if step <= 1 {
            return v;
        }
        (((v as u16 + step / 2) / step) * step).min(255) as u8
    }

    fn fg(&self, c: Rgb) -> String {
        match self.color {
            ColorDepth::Truecolor => {
                format!("\x1b[38;2;{};{};{}m", self.q(c.r), self.q(c.g), self.q(c.b))
            }
            _ => format!("\x1b[38;5;{}m", to_ansi256(c)),
        }
    }

    fn bg(&self, c: Rgb) -> String {
        match self.color {
            ColorDepth::Truecolor => {
                format!("\x1b[48;2;{};{};{}m", self.q(c.r), self.q(c.g), self.q(c.b))
            }
            _ => format!("\x1b[48;5;{}m", to_ansi256(c)),
        }
    }
}

impl Renderer for CellGradient {
    fn encode(&self, surface: &Surface) -> String {
        let rows = surface.height() / 2;
        let mut out = String::new();
        for cy in 0..rows {
            for x in 0..surface.width() {
                let top = surface.get(x, cy * 2);
                let bottom = surface.get(x, cy * 2 + 1);
                out.push_str(&self.fg(top));
                out.push_str(&self.bg(bottom));
                out.push('▀');
            }
            out.push_str(RESET);
            if cy + 1 < rows {
                out.push_str("\r\n");
            }
        }
        out
    }
}
