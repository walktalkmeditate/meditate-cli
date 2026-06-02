use std::time::Duration;

/// One phase of a breath cycle. `Still` is the open-focus "None" pattern, which
/// holds a single point and never advances or counts breaths.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Inhale,
    HoldIn,
    Exhale,
    HoldOut,
    Still,
}

impl Phase {
    pub fn label(self) -> &'static str {
        match self {
            Phase::Inhale => "inhale",
            Phase::HoldIn | Phase::HoldOut => "hold",
            Phase::Exhale => "exhale",
            Phase::Still => "be still",
        }
    }
}

/// A named breathing rhythm. Durations are in seconds and are lifted verbatim
/// from Pilgrim iOS `BreathRhythm.all`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pattern {
    pub name: &'static str,
    pub label: &'static str,
    pub inhale: f32,
    pub hold_in: f32,
    pub exhale: f32,
    pub hold_out: f32,
}

impl Pattern {
    pub fn cycle_len(&self) -> f32 {
        self.inhale + self.hold_in + self.exhale + self.hold_out
    }

    pub fn is_still(&self) -> bool {
        self.cycle_len() <= 0.0
    }
}

pub const PATTERNS: [Pattern; 7] = [
    Pattern {
        name: "calm",
        label: "5 / 7",
        inhale: 5.0,
        hold_in: 0.0,
        exhale: 7.0,
        hold_out: 0.0,
    },
    Pattern {
        name: "equal",
        label: "4 / 4",
        inhale: 4.0,
        hold_in: 0.0,
        exhale: 4.0,
        hold_out: 0.0,
    },
    Pattern {
        name: "relaxing",
        label: "4-7-8",
        inhale: 4.0,
        hold_in: 7.0,
        exhale: 8.0,
        hold_out: 0.0,
    },
    Pattern {
        name: "box",
        label: "4-4-4-4",
        inhale: 4.0,
        hold_in: 4.0,
        exhale: 4.0,
        hold_out: 4.0,
    },
    Pattern {
        name: "coherent",
        label: "5 / 5",
        inhale: 5.0,
        hold_in: 0.0,
        exhale: 5.0,
        hold_out: 0.0,
    },
    Pattern {
        name: "deep-calm",
        label: "3 / 6",
        inhale: 3.0,
        hold_in: 0.0,
        exhale: 6.0,
        hold_out: 0.0,
    },
    Pattern {
        name: "none",
        label: "open",
        inhale: 0.0,
        hold_in: 0.0,
        exhale: 0.0,
        hold_out: 0.0,
    },
];

/// Look up a pattern by name, clamping anything unknown to the first pattern —
/// matching the iOS `guard id >= 0 && id < count else return all[0]` behavior.
pub fn pattern_by_name(name: &str) -> Pattern {
    PATTERNS
        .iter()
        .copied()
        .find(|p| p.name == name)
        .unwrap_or(PATTERNS[0])
}

/// Milestone marks (in seconds) carried from iOS `milestoneSeconds`.
pub const MILESTONE_SECS: [u64; 5] = [300, 600, 900, 1200, 1800];

/// Width of the window a milestone can fire within. A frame-throttled loop may
/// not tick exactly on the second, so each mark is live for a span rather than
/// an instant (iOS uses `elapsed >= m && elapsed < m + 20`).
pub const MILESTONE_WINDOW_SECS: u64 = 20;

/// The milestone whose `[m, m + 20s)` window currently contains `elapsed_secs`,
/// if any. The caller is responsible for firing each milestone only once.
pub fn milestone_window(elapsed_secs: u64) -> Option<u64> {
    MILESTONE_SECS
        .iter()
        .copied()
        .find(|&m| elapsed_secs >= m && elapsed_secs < m + MILESTONE_WINDOW_SECS)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhaseState {
    pub phase: Phase,
    /// Progress through the current phase, 0.0 at its start to 1.0 at its end.
    pub progress: f32,
    pub breath_count: u32,
}

/// The breath engine. It is driven by a monotonic session clock the caller
/// supplies on each `tick`, so it carries no wall-clock dependency and is fully
/// deterministic to test. Pause/resume freeze the phase clock; switching a
/// pattern starts a fresh inhale and bumps a generation counter.
#[derive(Clone, Debug)]
pub struct Breath {
    pattern: Pattern,
    generation: u64,
    anchor: Duration,
    paused_since: Option<Duration>,
    paused_accum: Duration,
    breath_count: u32,
}

impl Breath {
    pub fn new(pattern: Pattern, now: Duration) -> Self {
        Breath {
            pattern,
            generation: 0,
            anchor: now,
            paused_since: None,
            paused_accum: Duration::ZERO,
            breath_count: 0,
        }
    }

    pub fn pattern(&self) -> Pattern {
        self.pattern
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn breath_count(&self) -> u32 {
        self.breath_count
    }

    pub fn is_paused(&self) -> bool {
        self.paused_since.is_some()
    }

    /// Switch to a new pattern, beginning a fresh inhale at `now`. The bumped
    /// generation lets a renderer ease the orb from its current scale into the
    /// new pattern rather than snapping.
    pub fn switch_to(&mut self, pattern: Pattern, now: Duration) {
        self.pattern = pattern;
        self.generation += 1;
        self.anchor = self.effective(now);
    }

    pub fn pause(&mut self, now: Duration) {
        if self.paused_since.is_none() {
            self.paused_since = Some(now);
        }
    }

    pub fn resume(&mut self, now: Duration) {
        if let Some(since) = self.paused_since.take() {
            self.paused_accum += now.saturating_sub(since);
        }
    }

    pub fn toggle_pause(&mut self, now: Duration) {
        if self.is_paused() {
            self.resume(now);
        } else {
            self.pause(now);
        }
    }

    /// Advance the engine to `now` and report the current phase. Whole cycles
    /// that have elapsed since the last tick increment the breath count.
    pub fn tick(&mut self, now: Duration) -> PhaseState {
        if self.pattern.is_still() {
            return PhaseState {
                phase: Phase::Still,
                progress: 0.0,
                breath_count: self.breath_count,
            };
        }

        let cycle = self.pattern.cycle_len();
        let mut within = self
            .effective(now)
            .saturating_sub(self.anchor)
            .as_secs_f32();
        while within >= cycle {
            self.anchor += Duration::from_secs_f32(cycle);
            self.breath_count += 1;
            within -= cycle;
        }

        let (phase, progress) = self.phase_at(within);
        PhaseState {
            phase,
            progress,
            breath_count: self.breath_count,
        }
    }

    /// Session time with paused spans removed, so the phase clock freezes while
    /// paused and resumes from the exact offset afterward.
    fn effective(&self, now: Duration) -> Duration {
        let ongoing = self
            .paused_since
            .map(|since| now.saturating_sub(since))
            .unwrap_or_default();
        now.saturating_sub(self.paused_accum)
            .saturating_sub(ongoing)
    }

    fn phase_at(&self, within: f32) -> (Phase, f32) {
        let p = self.pattern;
        let mut t = within;

        if t < p.inhale {
            return (Phase::Inhale, frac(t, p.inhale));
        }
        t -= p.inhale;

        if p.hold_in > 0.0 {
            if t < p.hold_in {
                return (Phase::HoldIn, frac(t, p.hold_in));
            }
            t -= p.hold_in;
        }

        if t < p.exhale {
            return (Phase::Exhale, frac(t, p.exhale));
        }
        t -= p.exhale;

        if p.hold_out <= 0.0 {
            return (Phase::Exhale, 1.0);
        }
        (Phase::HoldOut, frac(t, p.hold_out))
    }
}

fn frac(t: f32, len: f32) -> f32 {
    if len <= 0.0 {
        1.0
    } else {
        (t / len).clamp(0.0, 1.0)
    }
}
