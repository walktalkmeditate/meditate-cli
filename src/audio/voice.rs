use crate::pack::{AudioAsset, MeditationPrompt};

// Phase windows re-tuned for the CLI's shorter sessions. iOS originals (which
// would never fire in a sub-20-minute sit) are recorded for reference:
//   iOS settlingThresholdSec = 1200 (20 min), closingThresholdSec = 2700 (45 min)
pub const SETTLING_MAX_SECS: u64 = 180;
pub const CLOSING_MIN_SECS: u64 = 600;
pub const INITIAL_DELAY_SECS: u64 = 30;
pub const MIN_SPACING_SECS: u64 = 90;

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
pub fn has_meditation_prompts(asset: &AudioAsset) -> bool {
    !asset.meditation_prompts.is_empty()
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
}

impl VoiceScheduler {
    pub fn new(prompts: Vec<MeditationPrompt>) -> VoiceScheduler {
        VoiceScheduler {
            prompts,
            played: Vec::new(),
            last_emit: None,
            initial_delay: INITIAL_DELAY_SECS,
            spacing: MIN_SPACING_SECS,
        }
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
            .find(|prompt| !self.played.contains(&prompt.id) && phase_matches(&prompt.phase, phase))
            .cloned()?;

        self.played.push(pick.id.clone());
        self.last_emit = Some(elapsed_secs);
        Some(pick)
    }
}
