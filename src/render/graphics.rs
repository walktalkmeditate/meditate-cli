//! The orb as a real image, over a terminal inline-graphics protocol.
//!
//! The orb already rasterizes into an RGB [`Surface`]; here we paint it into a
//! higher-resolution, square-pixel surface and hand the pixels to the terminal
//! as an actual image, scaled into the orb's cell region (so the circle stays
//! round without us needing the cell pixel size).
//!
//! - [`KittyRenderer`] — the kitty graphics protocol (kitty, Ghostty, WezTerm),
//!   raw RGBA, no encoding dependency.
//! - [`ITerm2Renderer`] — iTerm2's OSC 1337 inline images, which want an image
//!   container; we emit a 24-bit BMP (NSImage decodes it, no zlib/CRC needed).

use crate::render::Surface;

/// kitty caps escape payloads at 4096 bytes per chunk.
const CHUNK: usize = 4096;

/// One kitty image id we reuse every frame, replacing its pixels in place.
const IMAGE_ID: u32 = 1;

/// Hard cap on the transmitted image's long edge. The protocol scales the image
/// into the cell region regardless, so a huge terminal never streams an enormous
/// frame.
const MAX_EDGE: usize = 512;

/// Supersample factor for the offscreen render: denser than the half-block grid
/// for a crisp image, capped so the transmitted frame stays small.
pub fn supersample(cols: usize, orb_rows: usize) -> usize {
    let span = cols.max(orb_rows * 2).max(1);
    (240 / span).clamp(2, 6)
}

/// Pixel dimensions of the offscreen surface for a `cols × orb_rows` cell region,
/// scaled down proportionally if the long edge would exceed `MAX_EDGE` (which
/// keeps the orb round and bounds the frame size on very large terminals).
pub fn surface_size(cols: usize, orb_rows: usize) -> (usize, usize) {
    let ss = supersample(cols, orb_rows);
    let (w, h) = (cols * ss, orb_rows * 2 * ss);
    let big = w.max(h);
    if big > MAX_EDGE {
        ((w * MAX_EDGE / big).max(1), (h * MAX_EDGE / big).max(1))
    } else {
        (w.max(1), h.max(1))
    }
}

/// A renderer that emits the orb image as terminal escapes.
pub trait ImageRenderer {
    /// Escapes that draw `surface` scaled to occupy `cols × rows` cells at the
    /// cursor, replacing the previous frame.
    fn frame(&self, surface: &Surface, cols: usize, rows: usize) -> String;
    /// Escapes that remove any lingering image on exit.
    fn teardown(&self) -> String;
}

// ── kitty graphics protocol ──────────────────────────────────────────────────

pub struct KittyRenderer;

impl KittyRenderer {
    pub fn new() -> KittyRenderer {
        KittyRenderer
    }
}

impl Default for KittyRenderer {
    fn default() -> KittyRenderer {
        KittyRenderer::new()
    }
}

impl ImageRenderer for KittyRenderer {
    fn frame(&self, surface: &Surface, cols: usize, rows: usize) -> String {
        let payload = base64(&rgba(surface));
        let mut out = String::new();
        // Drop the previous frame's placements so frames don't stack.
        out.push_str(&format!("\x1b_Ga=d,d=i,i={IMAGE_ID}\x1b\\"));
        // C=1 keeps the cursor put; q=2 silences kitty's acknowledgements (we
        // never read them back from raw mode).
        let keys = format!(
            "a=T,f=32,s={},v={},i={IMAGE_ID},c={cols},r={rows},C=1,q=2",
            surface.width(),
            surface.height()
        );
        emit_chunked(&mut out, &keys, &payload);
        out
    }

    fn teardown(&self) -> String {
        "\x1b_Ga=d,d=a\x1b\\".to_string()
    }
}

/// Write the payload as one or more chunked `_G` escapes. The first carries the
/// control keys; `m=1` marks "more follows", `m=0` the final chunk.
fn emit_chunked(out: &mut String, keys: &str, payload: &str) {
    let bytes = payload.as_bytes();
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

// ── iTerm2 OSC 1337 inline images ────────────────────────────────────────────

pub struct ITerm2Renderer;

impl ITerm2Renderer {
    pub fn new() -> ITerm2Renderer {
        ITerm2Renderer
    }
}

impl Default for ITerm2Renderer {
    fn default() -> ITerm2Renderer {
        ITerm2Renderer::new()
    }
}

impl ImageRenderer for ITerm2Renderer {
    fn frame(&self, surface: &Surface, cols: usize, rows: usize) -> String {
        let bmp = bmp_24(surface);
        let payload = base64(&bmp);
        format!(
            "\x1b]1337;File=inline=1;width={cols};height={rows};preserveAspectRatio=0;size={}:{payload}\x07",
            bmp.len()
        )
    }

    fn teardown(&self) -> String {
        // Inline images are grid content; leaving the alt screen clears them.
        String::new()
    }
}

/// A 24-bit, bottom-up BMP of the surface (BGR, rows padded to 4 bytes). No
/// compression, no checksums — the simplest container NSImage will decode.
fn bmp_24(surface: &Surface) -> Vec<u8> {
    let (w, h) = (surface.width(), surface.height());
    let row_bytes = w * 3;
    let padded = (row_bytes + 3) & !3;
    let pixels = padded * h;
    let file_size = 14 + 40 + pixels;

    let mut out = Vec::with_capacity(file_size);
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&(file_size as u32).to_le_bytes());
    out.extend_from_slice(&[0, 0, 0, 0]);
    out.extend_from_slice(&54u32.to_le_bytes()); // pixel-data offset = 14 + 40
    out.extend_from_slice(&40u32.to_le_bytes()); // BITMAPINFOHEADER size
    out.extend_from_slice(&(w as i32).to_le_bytes());
    out.extend_from_slice(&(h as i32).to_le_bytes()); // positive height = bottom-up
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&24u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB = uncompressed
    out.extend_from_slice(&(pixels as u32).to_le_bytes());
    out.extend_from_slice(&2835u32.to_le_bytes()); // 2835 ppm ≈ 72 DPI
    out.extend_from_slice(&2835u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());

    let pad = padded - row_bytes;
    for y in (0..h).rev() {
        for x in 0..w {
            let p = surface.get(x, y);
            out.extend_from_slice(&[p.b, p.g, p.r]);
        }
        out.extend(std::iter::repeat_n(0u8, pad));
    }
    out
}

// ── shared helpers ───────────────────────────────────────────────────────────

fn rgba(surface: &Surface) -> Vec<u8> {
    let (w, h) = (surface.width(), surface.height());
    let mut buf = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        for x in 0..w {
            let p = surface.get(x, y);
            buf.extend_from_slice(&[p.r, p.g, p.b, 255]);
        }
    }
    buf
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
    fn kitty_frame_deletes_then_transmits_and_places() {
        let mut surface = Surface::new(2, 2, Rgb::new(10, 20, 30));
        surface.set(0, 0, Rgb::new(255, 0, 0));
        let escapes = KittyRenderer::new().frame(&surface, 4, 3);

        let delete = escapes.find("a=d,d=i").expect("delete escape");
        let transmit = escapes.find("a=T").expect("transmit escape");
        assert!(delete < transmit, "must delete before placing");
        assert!(escapes.contains("s=2,v=2"));
        assert!(escapes.contains("c=4,r=3"));
        assert!(escapes.contains("f=32"));
        assert!(escapes.contains("\x1b_G"));
        assert!(escapes.ends_with("\x1b\\"));
    }

    #[test]
    fn kitty_large_frame_is_chunked() {
        let surface = Surface::new(64, 64, Rgb::new(1, 2, 3));
        let escapes = KittyRenderer::new().frame(&surface, 10, 5);
        assert!(escapes.contains("m=1"));
        assert!(escapes.contains("m=0"));
    }

    #[test]
    fn kitty_teardown_deletes_all_images() {
        assert_eq!(KittyRenderer::new().teardown(), "\x1b_Ga=d,d=a\x1b\\");
    }

    #[test]
    fn bmp_has_header_dimensions_and_size() {
        let surface = Surface::new(2, 3, Rgb::new(7, 8, 9));
        let bmp = bmp_24(&surface);
        assert_eq!(&bmp[0..2], b"BM");
        // width and height in the BITMAPINFOHEADER (offset 18 and 22).
        assert_eq!(i32::from_le_bytes(bmp[18..22].try_into().unwrap()), 2);
        assert_eq!(i32::from_le_bytes(bmp[22..26].try_into().unwrap()), 3);
        // 2px row = 6 bytes -> padded to 8; 8 * 3 rows + 54 header.
        assert_eq!(bmp.len(), 54 + 8 * 3);
        assert_eq!(
            u32::from_le_bytes(bmp[2..6].try_into().unwrap()) as usize,
            bmp.len()
        );
    }

    #[test]
    fn bmp_pixels_are_bottom_up_bgr() {
        let mut surface = Surface::new(1, 2, Rgb::new(0, 0, 0));
        surface.set(0, 0, Rgb::new(10, 20, 30)); // top row
        surface.set(0, 1, Rgb::new(40, 50, 60)); // bottom row
        let bmp = bmp_24(&surface);
        // Pixel data starts at offset 54; rows are 4-byte aligned (3 BGR + 1 pad).
        // BMP is bottom-up, so the file's first row is the surface's bottom row.
        assert_eq!(&bmp[54..57], &[60, 50, 40]); // bottom row, B G R
        assert_eq!(&bmp[58..61], &[30, 20, 10]); // top row, B G R
    }

    #[test]
    fn iterm2_frame_wraps_a_bmp_in_osc_1337() {
        let surface = Surface::new(2, 2, Rgb::new(1, 2, 3));
        let frame = ITerm2Renderer::new().frame(&surface, 4, 3);
        assert!(frame.starts_with("\x1b]1337;File=inline=1;"));
        assert!(frame.contains("width=4;height=3"));
        assert!(frame.contains("preserveAspectRatio=0"));
        assert!(frame.ends_with('\x07'));
        // The payload base64 starts with "Qk" — the encoding of the "BM" signature.
        assert!(frame.contains(":Qk"));
    }
}
