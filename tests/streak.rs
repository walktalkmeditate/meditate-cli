use meditate::streak::{record_session, reset, Streak, MIN_SESSION_SECS};
use std::sync::Arc;
use std::thread;

#[test]
fn records_and_continues_a_streak() {
    let mut streak = Streak::default();
    streak.record(100, 120);
    assert_eq!(streak.total_seconds, 120);
    assert_eq!(streak.current_streak, 1);
    assert_eq!(streak.last_day, Some(100));

    streak.record(101, 120);
    assert_eq!(streak.current_streak, 2);
    assert_eq!(streak.longest_streak, 2);

    streak.record(101, 60);
    assert_eq!(streak.current_streak, 2);
    assert_eq!(streak.total_seconds, 300);
}

#[test]
fn a_gap_resets_the_streak_but_keeps_the_longest() {
    let mut streak = Streak::default();
    streak.record(100, 120);
    streak.record(101, 120);
    streak.record(110, 120);
    assert_eq!(streak.current_streak, 1);
    assert_eq!(streak.longest_streak, 2);
}

#[test]
fn sub_minute_sessions_earn_nothing() {
    let mut streak = Streak::default();
    streak.record(100, MIN_SESSION_SECS - 1);
    assert_eq!(streak, Streak::default());
}

#[test]
fn load_tolerates_missing_and_corrupt_files() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(Streak::load_from(dir.path()), Streak::default());

    std::fs::write(Streak::path_in(dir.path()), "garbage = = =").unwrap();
    assert_eq!(Streak::load_from(dir.path()), Streak::default());
}

#[test]
fn reset_clears_and_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    record_session(dir.path(), 100, 120).unwrap();
    assert!(Streak::load_from(dir.path()).total_seconds > 0);

    reset(dir.path()).unwrap();
    assert_eq!(Streak::load_from(dir.path()), Streak::default());
    reset(dir.path()).unwrap();
}

#[test]
fn concurrent_sessions_both_count() {
    let dir = tempfile::tempdir().unwrap();
    let path = Arc::new(dir.path().to_path_buf());

    let handles: Vec<_> = (0..2)
        .map(|_| {
            let path = Arc::clone(&path);
            thread::spawn(move || record_session(&path, 200, 120).unwrap())
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }

    let streak = Streak::load_from(&path);
    assert_eq!(streak.total_seconds, 240);
    assert_eq!(streak.current_streak, 1);
}
