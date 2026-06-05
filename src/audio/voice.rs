use crate::pack::{MeditationPrompt, VoicePack};

// Phase windows re-tuned for the CLI's shorter sessions. iOS originals (which
// would never fire in a sub-20-minute sit) are recorded for reference:
//   iOS settlingThresholdSec = 1200 (20 min), closingThresholdSec = 2700 (45 min)
pub const SETTLING_MAX_SECS: u64 = 180;
pub const CLOSING_MIN_SECS: u64 = 600;
pub const INITIAL_DELAY_SECS: u64 = 30;
// Spacing between prompts jitters in this range, so the guide arrives with a
// gentle element of surprise rather than a fixed cadence — never rushed, never
// absent. The web applies the same jitter idea over a shorter range (its
// sessions skew shorter) — see VOICE_GAP_*_SECS in web/src/audio.ts.
pub const MIN_SPACING_SECS: u64 = 90;
pub const MAX_SPACING_SECS: u64 = 150;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VoicePhase {
    Settling,
    Deepening,
    Closing,
}

pub fn phase_name(phase: VoicePhase) -> &'static str {
    match phase {
        VoicePhase::Settling => "settling",
        VoicePhase::Deepening => "deepening",
        VoicePhase::Closing => "closing",
    }
}

pub fn prompt_phase(elapsed_secs: u64) -> VoicePhase {
    if elapsed_secs < SETTLING_MAX_SECS {
        VoicePhase::Settling
    } else if elapsed_secs < CLOSING_MIN_SECS {
        VoicePhase::Deepening
    } else {
        VoicePhase::Closing
    }
}

/// A voice pack is only offered if it carries meditation prompts. Walk prompts
/// are excluded structurally — the manifest model never deserializes them.
pub fn has_meditation_prompts(pack: &VoicePack) -> bool {
    !pack.meditation_prompts.is_empty()
}

fn phase_matches(prompt_phase: &str, current: VoicePhase) -> bool {
    prompt_phase.is_empty() || prompt_phase.eq_ignore_ascii_case(phase_name(current))
}

/// Schedules meditation voice prompts over a session: it honors an initial
/// delay and a minimum spacing, only plays prompts eligible for the current
/// elapsed-time phase, and never repeats a prompt.
pub struct VoiceScheduler {
    prompts: Vec<MeditationPrompt>,
    played: Vec<String>,
    last_emit: Option<u64>,
    initial_delay: u64,
    spacing: u64,
    rng: u64,
}

impl VoiceScheduler {
    pub fn new(prompts: Vec<MeditationPrompt>, seed: u64) -> VoiceScheduler {
        let mut scheduler = VoiceScheduler {
            prompts,
            played: Vec::new(),
            last_emit: None,
            initial_delay: INITIAL_DELAY_SECS,
            spacing: MIN_SPACING_SECS,
            rng: if seed == 0 {
                0x9E37_79B9_7F4A_7C15
            } else {
                seed
            },
        };
        scheduler.spacing = scheduler.jittered_spacing();
        scheduler
    }

    fn next_rand(&mut self) -> u64 {
        // xorshift64 — a tiny PRNG; we only need gentle jitter, not crypto.
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng = x;
        x
    }

    /// A spacing in `[MIN_SPACING_SECS, MAX_SPACING_SECS]`.
    fn jittered_spacing(&mut self) -> u64 {
        MIN_SPACING_SECS + self.next_rand() % (MAX_SPACING_SECS - MIN_SPACING_SECS + 1)
    }

    pub fn is_empty(&self) -> bool {
        self.prompts.is_empty()
    }

    /// The next prompt to play at `elapsed_secs`, or `None` if it isn't time yet
    /// or nothing eligible remains for the current phase.
    pub fn next(&mut self, elapsed_secs: u64) -> Option<MeditationPrompt> {
        if elapsed_secs < self.initial_delay {
            return None;
        }
        if let Some(last) = self.last_emit {
            if elapsed_secs < last + self.spacing {
                return None;
            }
        }

        let phase = prompt_phase(elapsed_secs);
        let pick = self
            .prompts
            .iter()
            .find(|prompt| {
                !self.played.contains(&prompt.id)
                    && phase_matches(prompt.phase.as_deref().unwrap_or(""), phase)
            })
            .cloned()?;

        self.played.push(pick.id.clone());
        self.last_emit = Some(elapsed_secs);
        self.spacing = self.jittered_spacing();
        Some(pick)
    }
}

/// A jitter seed from the wall clock, so each session's surprise differs. Falls
/// back to a constant when the clock is unavailable.
pub fn time_seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9E37_79B9_7F4A_7C15)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prompt(id: &str) -> MeditationPrompt {
        MeditationPrompt {
            id: id.to_string(),
            seq: 0,
            duration_sec: 0.0,
            file_size_bytes: 0,
            r2_key: String::new(),
            phase: None,
        }
    }

    #[test]
    fn spacing_jitter_stays_within_bounds() {
        let mut s = VoiceScheduler::new(vec![prompt("a")], 12345);
        for _ in 0..200 {
            let gap = s.jittered_spacing();
            assert!((MIN_SPACING_SECS..=MAX_SPACING_SECS).contains(&gap));
        }
    }

    #[test]
    fn next_waits_for_initial_delay_then_respects_jittered_spacing() {
        let mut s = VoiceScheduler::new(vec![prompt("a"), prompt("b"), prompt("c")], 777);
        // Nothing before the initial delay.
        assert!(s.next(INITIAL_DELAY_SECS - 1).is_none());
        // First prompt fires once the delay passes.
        let first = s.next(INITIAL_DELAY_SECS).expect("first prompt");
        // The next is gated by at least MIN_SPACING_SECS, never sooner.
        assert!(s.next(INITIAL_DELAY_SECS + MIN_SPACING_SECS - 1).is_none());
        // Far enough out, the second (different) prompt fires.
        let second = s
            .next(INITIAL_DELAY_SECS + MAX_SPACING_SECS)
            .expect("second prompt");
        assert_ne!(first.id, second.id);
    }
}
