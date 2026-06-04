//! The browser façade over `meditate-core`.
//!
//! JS holds one opaque [`Session`] and, each animation frame, calls
//! [`Session::tick_frame`] with the elapsed session time and the terminal size;
//! it gets back the ANSI for the half-block orb (plus the OSC-0 title bytes,
//! which xterm.js turns into the breathing browser-tab title). The smooth orb
//! (a Canvas-2D overlay) is driven from the cheap breath-state accessors —
//! `fullness`, `glow`, `palette` — so there is no per-frame pixel buffer
//! crossing the wasm boundary.
//!
//! All state lives here; the boundary is crossed once per frame for the ANSI
//! string and otherwise only for small scalars.

use meditate_core::breath::{self, Breath, Phase, PhaseState};
use meditate_core::palette::{self, season_for_month, time_for_hour, Palette};
use meditate_core::render::cell_gradient::CellGradient;
use meditate_core::render::orb::{self, OrbScene};
use meditate_core::render::Surface;
use meditate_core::render::{Rgb, Renderer};
use meditate_core::title;
use std::time::Duration;
use wasm_bindgen::prelude::*;

/// A live breathing session: a breath engine plus the resolved palette and the
/// truecolor half-block renderer (the web terminal is always truecolor).
#[wasm_bindgen]
pub struct Session {
    breath: Breath,
    renderer: CellGradient,
    palette: Palette,
    last_state: PhaseState,
    last_title: String,
    last_now: Duration,
}

#[wasm_bindgen]
impl Session {
    /// Open a session on `pattern` (unknown names fall back to the first
    /// pattern, matching the CLI). `month` (1–12) and `hour` (0–23) come from
    /// the browser clock and pick the seasonal / time-of-day palette — the core
    /// has no clock of its own.
    #[wasm_bindgen(constructor)]
    pub fn new(pattern: &str, month: u32, hour: u32) -> Session {
        let palette = palette::resolve_with_pin(season_for_month(month), time_for_hour(hour), None);
        let mut breath = Breath::new(breath::pattern_by_name(pattern), Duration::ZERO);
        let last_state = breath.tick(Duration::ZERO);
        Session {
            breath,
            renderer: CellGradient::new(meditate_core::caps::ColorDepth::Truecolor),
            palette,
            last_state,
            last_title: String::new(),
            last_now: Duration::ZERO,
        }
    }

    /// Switch breathing pattern, easing from the current breath into a fresh
    /// inhale (uses the most recent tick time).
    #[wasm_bindgen(js_name = setPattern)]
    pub fn set_pattern(&mut self, name: &str) {
        self.breath
            .switch_to(breath::pattern_by_name(name), self.last_now);
    }

    /// Freeze or resume the breath clock.
    #[wasm_bindgen(js_name = pauseToggle)]
    pub fn pause_toggle(&mut self) {
        self.breath.toggle_pause(self.last_now);
    }

    #[wasm_bindgen(js_name = isPaused)]
    pub fn is_paused(&self) -> bool {
        self.breath.is_paused()
    }

    /// Advance to `elapsed_ms` of session time and return the ANSI to draw the
    /// orb into a `cols × rows` cell region (two pixel rows per cell). When the
    /// title line changes, the OSC-0 sequence is prepended so xterm.js animates
    /// the browser tab. The caller wraps this in synchronized-output and homes
    /// the cursor; this returns only the frame's content.
    #[wasm_bindgen(js_name = tickFrame)]
    pub fn tick_frame(&mut self, elapsed_ms: f64, cols: u32, rows: u32) -> String {
        let now = Duration::from_secs_f64((elapsed_ms.max(0.0)) / 1000.0);
        self.last_now = now;
        let state = self.breath.tick(now);
        self.last_state = state;

        let (cols, rows) = (cols as usize, rows as usize);
        if cols == 0 || rows == 0 {
            return String::new();
        }

        let scene = OrbScene {
            scale: orb::scale_for(state),
            glow: orb::glow_for(state),
            ripples: Vec::new(),
            milestone_flash: 0.0,
            palette: self.palette,
        };
        let mut surface = Surface::new(cols, rows * 2, self.palette.background);
        orb::paint(&mut surface, &scene);

        let mut out = String::new();
        let title = title::breath_title(state);
        if title != self.last_title {
            out.push_str(&title::set_sequence(&title));
            self.last_title = title;
        }
        out.push_str(&self.renderer.encode(&surface));
        out
    }

    /// Breath fullness in `0.0..=1.0` (empty at the bottom of the exhale, full at
    /// the top of the inhale) — the smooth-orb canvas reads this for the radius.
    pub fn fullness(&self) -> f32 {
        title::fullness(self.last_state)
    }

    /// Inner-glow intensity for the current phase (brightest on a held breath).
    pub fn glow(&self) -> f32 {
        orb::glow_for(self.last_state)
    }

    /// Orb scale in `0.45..=1.0` for the current phase.
    pub fn scale(&self) -> f32 {
        orb::scale_for(self.last_state)
    }

    #[wasm_bindgen(js_name = phaseLabel)]
    pub fn phase_label(&self) -> String {
        self.last_state.phase.label().to_string()
    }

    #[wasm_bindgen(js_name = isStill)]
    pub fn is_still(&self) -> bool {
        self.last_state.phase == Phase::Still
    }

    #[wasm_bindgen(js_name = breathCount)]
    pub fn breath_count(&self) -> u32 {
        self.breath.breath_count()
    }

    /// The orb's three colors as 9 bytes — core RGB, edge RGB, ripple RGB — for
    /// the Canvas-2D gradient (so the smooth orb matches the terminal orb's
    /// season/time palette exactly).
    pub fn palette(&self) -> Vec<u8> {
        let p = self.palette;
        let push = |v: &mut Vec<u8>, c: Rgb| {
            v.push(c.r);
            v.push(c.g);
            v.push(c.b);
        };
        let mut out = Vec::with_capacity(9);
        push(&mut out, p.core);
        push(&mut out, p.edge);
        push(&mut out, p.ripple);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_frame_emits_truecolor_halfblock_and_title() {
        let mut session = Session::new("box", 6, 12);
        let frame = session.tick_frame(0.0, 20, 10);
        assert!(frame.contains('▀'), "expected half-block cells");
        assert!(frame.contains("\x1b[38;2;"), "expected a truecolor fg escape");
        assert!(
            frame.contains("\x1b]0;") && frame.contains("meditate"),
            "expected the OSC-0 breathing title on the first frame"
        );
    }

    #[test]
    fn fullness_rises_on_inhale_and_falls_on_exhale() {
        // box = 4-4-4-4: inhale [0,4)s, hold [4,8), exhale [8,12), hold [12,16).
        let mut session = Session::new("box", 6, 12);
        session.tick_frame(500.0, 20, 10);
        let early_inhale = session.fullness();
        session.tick_frame(3500.0, 20, 10);
        let late_inhale = session.fullness();
        assert!(
            late_inhale > early_inhale,
            "fullness should rise across the inhale ({early_inhale} -> {late_inhale})"
        );

        session.tick_frame(8500.0, 20, 10);
        let early_exhale = session.fullness();
        session.tick_frame(11500.0, 20, 10);
        let late_exhale = session.fullness();
        assert!(
            late_exhale < early_exhale,
            "fullness should fall across the exhale ({early_exhale} -> {late_exhale})"
        );
    }

    #[test]
    fn palette_returns_nine_bytes_and_set_pattern_holds() {
        let mut session = Session::new("calm", 1, 23);
        assert_eq!(session.palette().len(), 9);
        // Unknown pattern falls back without panicking; still renders.
        session.set_pattern("not-a-pattern");
        let frame = session.tick_frame(100.0, 12, 6);
        assert!(!frame.is_empty());
    }
}
