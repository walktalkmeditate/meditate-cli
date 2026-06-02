use super::{Renderer, Surface};

/// Renders the surface without color, mapping brightness to a five-step block
/// ramp. Used on 16-color, NO_COLOR, and dumb terminals.
pub struct Mono;

const RAMP: [char; 5] = [' ', '░', '▒', '▓', '█'];

impl Renderer for Mono {
    fn encode(&self, surface: &Surface) -> String {
        let rows = surface.height() / 2;
        let mut out = String::new();
        for cy in 0..rows {
            for x in 0..surface.width() {
                let top = surface.get(x, cy * 2).luma();
                let bottom = surface.get(x, cy * 2 + 1).luma();
                out.push(ramp_char((top + bottom) / 2.0));
            }
            if cy + 1 < rows {
                out.push_str("\r\n");
            }
        }
        out
    }
}

fn ramp_char(luma: f32) -> char {
    let index = (luma.clamp(0.0, 1.0) * (RAMP.len() - 1) as f32).round() as usize;
    RAMP[index]
}
