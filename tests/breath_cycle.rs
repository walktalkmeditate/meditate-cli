use meditate::breath::{
    milestone_window, pattern_by_name, Breath, Phase, MILESTONE_SECS, PATTERNS,
};
use std::time::Duration;

fn at(secs: f32) -> Duration {
    Duration::from_secs_f32(secs)
}

fn assert_close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < 1e-3,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn ships_seven_patterns_with_verified_timings() {
    assert_eq!(PATTERNS.len(), 7);
    let names: Vec<_> = PATTERNS.iter().map(|p| p.name).collect();
    assert_eq!(
        names,
        [
            "calm",
            "equal",
            "relaxing",
            "box",
            "coherent",
            "deep-calm",
            "none"
        ]
    );

    let box_pattern = pattern_by_name("box");
    assert_eq!(box_pattern.cycle_len(), 16.0);
    let relaxing = pattern_by_name("relaxing");
    assert_eq!(
        (relaxing.inhale, relaxing.hold_in, relaxing.exhale),
        (4.0, 7.0, 8.0)
    );
}

#[test]
fn unknown_pattern_clamps_to_first() {
    assert_eq!(pattern_by_name("wobble").name, "calm");
    assert_eq!(pattern_by_name("box").name, "box");
}

#[test]
fn box_pattern_walks_all_four_phases() {
    let mut breath = Breath::new(pattern_by_name("box"), at(0.0));
    assert_eq!(breath.tick(at(2.0)).phase, Phase::Inhale);
    assert_eq!(breath.tick(at(6.0)).phase, Phase::HoldIn);
    assert_eq!(breath.tick(at(10.0)).phase, Phase::Exhale);
    assert_eq!(breath.tick(at(14.0)).phase, Phase::HoldOut);

    let next = breath.tick(at(17.0));
    assert_eq!(next.phase, Phase::Inhale);
    assert_eq!(next.breath_count, 1);
    assert_close(next.progress, 0.25);
}

#[test]
fn calm_pattern_skips_holds() {
    let mut breath = Breath::new(pattern_by_name("calm"), at(0.0));
    let inhale = breath.tick(at(2.0));
    assert_eq!(inhale.phase, Phase::Inhale);
    assert_close(inhale.progress, 0.4);

    assert_eq!(breath.tick(at(6.0)).phase, Phase::Exhale);
    assert_eq!(breath.tick(at(13.0)).breath_count, 1);
}

#[test]
fn none_pattern_is_a_still_point_that_never_counts() {
    let mut breath = Breath::new(pattern_by_name("none"), at(0.0));
    assert_eq!(breath.tick(at(5.0)).phase, Phase::Still);
    let later = breath.tick(at(600.0));
    assert_eq!(later.phase, Phase::Still);
    assert_eq!(later.breath_count, 0);
}

#[test]
fn mid_phase_switch_bumps_generation_and_restarts_inhale() {
    let mut breath = Breath::new(pattern_by_name("box"), at(0.0));
    assert_eq!(breath.tick(at(6.0)).phase, Phase::HoldIn);

    breath.switch_to(pattern_by_name("calm"), at(6.0));
    assert_eq!(breath.generation(), 1);

    let immediately = breath.tick(at(6.0));
    assert_eq!(immediately.phase, Phase::Inhale);
    assert_close(immediately.progress, 0.0);

    let later = breath.tick(at(8.0));
    assert_eq!(later.phase, Phase::Inhale);
    assert_close(later.progress, 0.4);
}

#[test]
fn pause_freezes_the_phase_clock_and_resume_continues_from_offset() {
    let mut breath = Breath::new(pattern_by_name("calm"), at(0.0));
    assert_close(breath.tick(at(3.0)).progress, 0.6);

    breath.pause(at(3.0));
    assert!(breath.is_paused());
    let while_paused = breath.tick(at(10.0));
    assert_eq!(while_paused.phase, Phase::Inhale);
    assert_close(while_paused.progress, 0.6);

    breath.resume(at(10.0));
    assert!(!breath.is_paused());
    assert_close(breath.tick(at(11.0)).progress, 0.8);
}

#[test]
fn pause_spanning_a_boundary_does_not_skip_or_over_count() {
    let mut breath = Breath::new(pattern_by_name("equal"), at(0.0));
    breath.tick(at(3.0));
    breath.pause(at(3.0));
    breath.resume(at(100.0));

    let resumed = breath.tick(at(105.0));
    assert_eq!(resumed.phase, Phase::Inhale);
    assert_eq!(resumed.breath_count, 1);
}

#[test]
fn milestones_fire_within_a_twenty_second_window() {
    assert_eq!(milestone_window(299), None);
    assert_eq!(milestone_window(300), Some(300));
    assert_eq!(milestone_window(315), Some(300));
    assert_eq!(milestone_window(320), None);
    assert_eq!(milestone_window(600), Some(600));
    assert_eq!(MILESTONE_SECS, [300, 600, 900, 1200, 1800]);
}
