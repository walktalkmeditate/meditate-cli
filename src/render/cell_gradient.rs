use super::{to_ansi256, Renderer, Rgb, Surface, RESET};
use crate::term::ColorDepth;

/// Renders the surface as truecolor (or 256-color) half-block cells: each cell
/// is `▀`, with the foreground painting the top pixel and the background the
/// bottom pixel, so one character row shows two pixel rows.
pub struct CellGradient {
    color: ColorDepth,
}

impl CellGradient {
    pub fn new(color: ColorDepth) -> CellGradient {
        CellGradient { color }
    }

    fn fg(&self, c: Rgb) -> String {
        match self.color {
            ColorDepth::Truecolor => format!("\x1b[38;2;{};{};{}m", c.r, c.g, c.b),
            _ => format!("\x1b[38;5;{}m", to_ansi256(c)),
        }
    }

    fn bg(&self, c: Rgb) -> String {
        match self.color {
            ColorDepth::Truecolor => format!("\x1b[48;2;{};{};{}m", c.r, c.g, c.b),
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
