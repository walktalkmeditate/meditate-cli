use meditate::config::Config;
use meditate::door;
use meditate::session::{
    end_mode, parse_duration, reduce_motion_enabled, should_end, ymd_from_unix_days, EndMode,
    MilestoneTracker,
};
use meditate::term::MapEnv;
use std::time::Duration;

#[test]
fn parses_durations() {
    assert_eq!(parse_duration("90s"), Some(Duration::from_secs(90)));
    assert_eq!(parse_duration("5m"), Some(Duration::from_secs(300)));
    assert_eq!(parse_duration("1h30m"), Some(Duration::from_secs(5400)));
    assert_eq!(parse_duration("5m30s"), Some(Duration::from_secs(330)));
    assert_eq!(parse_duration("120"), Some(Duration::from_secs(120)));
    assert_eq!(parse_duration("0"), None);
    assert_eq!(parse_duration(""), None);
    assert_eq!(parse_duration("5x"), None);
    assert_eq!(parse_duration("9223372036854775807h"), None); // overflow guard
}

#[test]
fn end_mode_prefers_breaths_then_duration() {
    assert_eq!(end_mode(None, Some(10)), Some(EndMode::Breaths(10)));
    assert_eq!(
        end_mode(Some("5m"), None),
        Some(EndMode::After(Duration::from_secs(300)))
    );
    assert_eq!(end_mode(None, None), Some(EndMode::OpenEnded));
    assert_eq!(end_mode(Some("nonsense"), None), None);
}

#[test]
fn should_end_respects_each_mode() {
    assert!(!should_end(
        EndMode::OpenEnded,
        Duration::from_secs(9999),
        999
    ));

    assert!(should_end(
        EndMode::After(Duration::from_secs(60)),
        Duration::from_secs(60),
        0
    ));
    assert!(!should_end(
        EndMode::After(Duration::from_secs(60)),
        Duration::from_secs(59),
        0
    ));

    assert!(should_end(EndMode::Breaths(10), Duration::ZERO, 10));
    assert!(!should_end(EndMode::Breaths(10), Duration::ZERO, 9));
}

#[test]
fn milestones_fire_once_each() {
    let mut tracker = MilestoneTracker::new();
    assert_eq!(tracker.check(300), Some(300));
    assert_eq!(tracker.check(305), None);
    assert_eq!(tracker.check(599), None);
    assert_eq!(tracker.check(600), Some(600));
}

#[test]
fn civil_date_from_unix_days() {
    assert_eq!(ymd_from_unix_days(0), (1970, 1, 1));
    assert_eq!(ymd_from_unix_days(18628), (2021, 1, 1));
}

#[test]
fn reduce_motion_from_flag_config_or_env() {
    let config = Config::default();
    let empty = MapEnv::new(&[]);
    assert!(reduce_motion_enabled(true, &config, &empty));

    let motion_config = Config {
        reduce_motion: Some(true),
        ..Config::default()
    };
    assert!(reduce_motion_enabled(false, &motion_config, &empty));

    let env = MapEnv::new(&[("REDUCE_MOTION", "1")]);
    assert!(reduce_motion_enabled(false, &config, &env));

    assert!(!reduce_motion_enabled(false, &config, &empty));
}

#[test]
fn pilgrim_door_only_after_a_long_sit() {
    assert!(door::should_show(
        Duration::from_secs(600),
        door::DEFAULT_LONG_SESSION,
        true
    ));
    assert!(!door::should_show(
        Duration::from_secs(599),
        door::DEFAULT_LONG_SESSION,
        true
    ));
    assert!(!door::should_show(
        Duration::from_secs(900),
        door::DEFAULT_LONG_SESSION,
        false
    ));
}
