//! Mirror the breath into the terminal tab/window title.
//!
//! The title is plain text, so the breath shows as a block that rises on the
//! inhale and falls on the exhale. The block ramp `▁▂▃▄▅▆▇█` is a single
//! designed family, so it keeps a constant cell width and baseline in every
//! font — no jiggle across terminals. Driven over OSC, the tab keeps animating
//! even while it sits in the background behind another tab.

use crate::breath::{Phase, PhaseState};

/// Lower-block ramp, U+2581..U+2588. A coherently-designed set, so every step
/// shares a width and baseline.
const RAMP: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// Breath fullness in `0.0..=1.0`: empty at the bottom of the exhale, full at
/// the top of the inhale. Holds sit at their extreme; the still pattern rests
/// at a steady middle.
pub fn fullness(state: PhaseState) -> f32 {
    match state.phase {
        Phase::Inhale => state.progress,
        Phase::HoldIn => 1.0,
        Phase::Exhale => 1.0 - state.progress,
        Phase::HoldOut => 0.0,
        Phase::Still => 0.5,
    }
}

/// The ramp block for a fullness value.
pub fn block_for(fullness: f32) -> char {
    let last = RAMP.len() - 1;
    let index = (fullness.clamp(0.0, 1.0) * last as f32).round() as usize;
    RAMP[index.min(last)]
}

/// The title line for a breath state, e.g. `"▄ meditate · inhale"`.
pub fn breath_title(state: PhaseState) -> String {
    format!(
        "{} meditate · {}",
        block_for(fullness(state)),
        state.phase.label()
    )
}

/// OSC 0 sets both the icon and window title; terminals mirror one of them into
/// the tab label, so setting both is the most portable choice.
pub fn set_sequence(title: &str) -> String {
    format!("\x1b]0;{title}\x07")
}

/// Save both titles on the terminal's title stack (xterm XTWINOPS, supported by
/// kitty and most modern terminals). Paired with [`POP_TITLE`] to restore the
/// user's original tab name on exit.
pub const PUSH_TITLE: &str = "\x1b[22;0t";

/// Restore both titles from the stack.
pub const POP_TITLE: &str = "\x1b[23;0t";

#[cfg(test)]
mod tests {
    use super::*;

    fn state(phase: Phase, progress: f32) -> PhaseState {
        PhaseState {
            phase,
            progress,
            breath_count: 0,
        }
    }

    #[test]
    fn fullness_rises_on_inhale_and_falls_on_exhale() {
        assert_eq!(fullness(state(Phase::Inhale, 0.0)), 0.0);
        assert_eq!(fullness(state(Phase::Inhale, 1.0)), 1.0);
        assert_eq!(fullness(state(Phase::HoldIn, 0.5)), 1.0);
        assert_eq!(fullness(state(Phase::Exhale, 0.0)), 1.0);
        assert_eq!(fullness(state(Phase::Exhale, 1.0)), 0.0);
        assert_eq!(fullness(state(Phase::HoldOut, 0.5)), 0.0);
        assert_eq!(fullness(state(Phase::Still, 0.0)), 0.5);
    }

    #[test]
    fn block_spans_the_ramp_and_clamps() {
        assert_eq!(block_for(0.0), '▁');
        assert_eq!(block_for(1.0), '█');
        assert_eq!(block_for(-1.0), '▁');
        assert_eq!(block_for(2.0), '█');
        assert_eq!(block_for(0.5), '▅');
    }

    #[test]
    fn title_reads_block_then_phase() {
        assert_eq!(
            breath_title(state(Phase::Inhale, 0.0)),
            "▁ meditate · inhale"
        );
        assert_eq!(breath_title(state(Phase::HoldIn, 0.0)), "█ meditate · hold");
    }

    #[test]
    fn set_sequence_wraps_in_osc() {
        assert_eq!(set_sequence("hi"), "\x1b]0;hi\x07");
    }
}
