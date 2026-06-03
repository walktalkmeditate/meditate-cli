use meditate::config::Config;
use meditate::paths::validate_base_dir;
use meditate::resolve_start_pattern;
use meditate::state::State;
use std::path::Path;

#[test]
fn missing_config_loads_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let config = Config::load_from(dir.path()).unwrap();
    assert_eq!(config, Config::default());
    assert!(config.resume_last_pattern());
}

#[test]
fn config_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let config = Config {
        default_pattern: Some("box".into()),
        master_volume: Some(60),
        palette: Some("auto".into()),
        ..Config::default()
    };
    config.save_to(dir.path()).unwrap();
    assert_eq!(Config::load_from(dir.path()).unwrap(), config);
}

#[test]
fn malformed_config_errors_but_defaults_recover() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(Config::path_in(dir.path()), "this is = not = valid").unwrap();
    assert!(Config::load_from(dir.path()).is_err());
    assert_eq!(Config::load_or_default(dir.path()), Config::default());
}

#[test]
fn start_pattern_arbitration() {
    let pinned = Config {
        default_pattern: Some("calm".into()),
        ..Config::default()
    };
    let state = State {
        last_pattern: Some("box".into()),
        ..State::default()
    };

    // The command line always wins.
    assert_eq!(
        resolve_start_pattern(Some("coherent"), &pinned, &state),
        Some("coherent".into())
    );
    // A pinned config default beats the remembered pattern.
    assert_eq!(
        resolve_start_pattern(None, &pinned, &state),
        Some("calm".into())
    );
    // With nothing pinned, the remembered pattern is resumed.
    assert_eq!(
        resolve_start_pattern(None, &Config::default(), &state),
        Some("box".into())
    );
    // resume_last_pattern = false disables session memory.
    let no_resume = Config {
        resume_last_pattern: Some(false),
        ..Config::default()
    };
    assert_eq!(resolve_start_pattern(None, &no_resume, &state), None);
}

#[test]
fn config_template_writes_and_loads_as_defaults() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        Config::path_in(dir.path()),
        meditate::config::default_template(),
    )
    .unwrap();
    // Every line is commented, so a freshly generated config equals the defaults.
    assert_eq!(Config::load_from(dir.path()).unwrap(), Config::default());

    let template = meditate::config::default_template();
    for key in [
        "default_pattern",
        "resume_last_pattern",
        "master_volume",
        "reduce_motion",
        "default_soundscape",
        "default_voice",
        "default_bell",
        "streak_enabled",
        "door_enabled",
        "tab_title",
        "graphics",
        "[keymap]",
    ] {
        assert!(template.contains(key), "template missing `{key}`");
    }
}

#[test]
fn config_round_trips_new_toggles() {
    let dir = tempfile::tempdir().unwrap();
    let config = Config {
        tab_title: Some(true),
        graphics: Some(false),
        ..Config::default()
    };
    config.save_to(dir.path()).unwrap();
    assert_eq!(Config::load_from(dir.path()).unwrap(), config);
}

#[test]
fn start_pattern_is_none_without_any_source() {
    assert_eq!(
        resolve_start_pattern(None, &Config::default(), &State::default()),
        None
    );
}

#[test]
fn validate_base_dir_rejects_unsafe_paths() {
    assert!(validate_base_dir(Path::new("relative/dir")).is_err());
    assert!(validate_base_dir(Path::new("/var/data/../../etc")).is_err());

    let dir = tempfile::tempdir().unwrap();
    assert!(validate_base_dir(dir.path()).is_ok());
}

#[test]
fn state_round_trips_and_tolerates_corruption() {
    let dir = tempfile::tempdir().unwrap();
    let state = State {
        last_pattern: Some("equal".into()),
        master_volume: Some(70),
        soundscape: Some("forest".into()),
        voice: None,
        bell: Some("echo-chime".into()),
    };
    state.save_to(dir.path()).unwrap();
    assert_eq!(State::load_from(dir.path()), state);

    std::fs::write(State::path_in(dir.path()), "garbage = = =").unwrap();
    assert_eq!(State::load_from(dir.path()), State::default());
}
