//! The orb as a real image, over the kitty graphics protocol.
//!
//! The orb already rasterizes into an RGB [`Surface`]; here we paint it into a
//! higher-resolution, square-pixel surface and hand the pixels to the terminal
//! as an actual image. The kitty protocol scales the image into the orb's cell
//! region, so the circle stays round without us needing the cell pixel size.
//! kitty, Ghostty, and WezTerm all speak this protocol.

use crate::render::Surface;

/// kitty caps escape payloads at 4096 bytes per chunk.
const CHUNK: usize = 4096;

/// One image id we reuse every frame, replacing its pixels in place.
const IMAGE_ID: u32 = 1;

/// Supersample factor for the offscreen render: denser than the half-block grid
/// for a crisp image, capped so the transmitted frame stays small.
pub fn supersample(cols: usize, orb_rows: usize) -> usize {
    let span = cols.max(orb_rows * 2).max(1);
    (240 / span).clamp(2, 6)
}

/// Pixel dimensions of the offscreen surface for a `cols × orb_rows` cell region.
pub fn surface_size(cols: usize, orb_rows: usize) -> (usize, usize) {
    let ss = supersample(cols, orb_rows);
    (cols * ss, orb_rows * 2 * ss)
}

pub struct KittyRenderer;

impl KittyRenderer {
    pub fn new() -> KittyRenderer {
        KittyRenderer
    }

    /// Escapes that transmit `surface` as RGBA and place it, scaled to occupy
    /// `cols × rows` cells at the cursor, replacing the previous frame.
    pub fn frame(&self, surface: &Surface, cols: usize, rows: usize) -> String {
        let (w, h) = (surface.width(), surface.height());
        let mut rgba = Vec::with_capacity(w * h * 4);
        for y in 0..h {
            for x in 0..w {
                let p = surface.get(x, y);
                rgba.extend_from_slice(&[p.r, p.g, p.b, 255]);
            }
        }
        let payload = base64(&rgba);

        let mut out = String::new();
        // Drop the previous frame's placements so frames don't stack.
        out.push_str(&format!("\x1b_Ga=d,d=i,i={IMAGE_ID}\x1b\\"));
        // C=1 keeps the cursor put; q=2 silences kitty's acknowledgements (we
        // never read them back from raw mode).
        let keys = format!("a=T,f=32,s={w},v={h},i={IMAGE_ID},c={cols},r={rows},C=1,q=2");
        emit_chunked(&mut out, &keys, &payload);
        out
    }
}

impl Default for KittyRenderer {
    fn default() -> KittyRenderer {
        KittyRenderer::new()
    }
}

/// Remove every image — used on teardown so nothing lingers on the screen.
pub fn teardown() -> String {
    "\x1b_Ga=d,d=a\x1b\\".to_string()
}

/// Write the payload as one or more chunked `_G` escapes. The first carries the
/// control keys; `m=1` marks "more follows", `m=0` the final chunk.
fn emit_chunked(out: &mut String, keys: &str, payload: &str) {
    let bytes = payload.as_bytes();
    if bytes.is_empty() {
        out.push_str(&format!("\x1b_G{keys},m=0;\x1b\\"));
        return;
    }
    let mut start = 0;
    let mut first = true;
    while start < bytes.len() {
        let end = (start + CHUNK).min(bytes.len());
        let chunk = &payload[start..end];
        let more = u8::from(end < bytes.len());
        if first {
            out.push_str(&format!("\x1b_G{keys},m={more};{chunk}\x1b\\"));
            first = false;
        } else {
            out.push_str(&format!("\x1b_Gm={more};{chunk}\x1b\\"));
        }
        start = end;
    }
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64, hand-rolled to avoid a dependency.
fn base64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(B64[(n >> 18 & 63) as usize] as char);
        out.push(B64[(n >> 12 & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            B64[(n >> 6 & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            B64[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::Rgb;

    #[test]
    fn base64_matches_known_vectors() {
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"M"), "TQ==");
        assert_eq!(base64(b""), "");
    }

    #[test]
    fn supersample_is_bounded() {
        assert!((2..=6).contains(&supersample(80, 23)));
        assert_eq!(supersample(1000, 1000), 2);
        assert_eq!(supersample(1, 1), 6);
    }

    #[test]
    fn frame_deletes_then_transmits_and_places() {
        let mut surface = Surface::new(2, 2, Rgb::new(10, 20, 30));
        surface.set(0, 0, Rgb::new(255, 0, 0));
        let escapes = KittyRenderer::new().frame(&surface, 4, 3);

        // Clears the prior placement before transmitting.
        let delete = escapes.find("a=d,d=i").expect("delete escape");
        let transmit = escapes.find("a=T").expect("transmit escape");
        assert!(delete < transmit, "must delete before placing");

        // Carries the size, image id, and cell-region placement keys.
        assert!(escapes.contains("s=2,v=2"));
        assert!(escapes.contains("c=4,r=3"));
        assert!(escapes.contains("f=32"));
        // Properly framed APC escape.
        assert!(escapes.contains("\x1b_G"));
        assert!(escapes.ends_with("\x1b\\"));
    }

    #[test]
    fn large_frame_is_chunked() {
        let surface = Surface::new(64, 64, Rgb::new(1, 2, 3));
        let escapes = KittyRenderer::new().frame(&surface, 10, 5);
        // 64*64*4 bytes -> base64 well over one 4096-byte chunk -> m=1 appears.
        assert!(escapes.contains("m=1"));
        assert!(escapes.contains("m=0"));
    }

    #[test]
    fn teardown_deletes_all_images() {
        assert_eq!(teardown(), "\x1b_Ga=d,d=a\x1b\\");
    }
}
