use meditate::config::Config;
use meditate::keymap::{Action, Keymap};

#[test]
fn default_bindings_cover_the_core_controls() {
    let keymap = Keymap::default();
    assert_eq!(keymap.action_for('q'), Some(Action::Quit));
    assert_eq!(keymap.action_for(' '), Some(Action::Pause));
    assert_eq!(keymap.action_for('n'), Some(Action::NextPattern));
    assert_eq!(keymap.action_for('b'), Some(Action::ToggleBell));
    assert_eq!(keymap.action_for('m'), Some(Action::Mute));
    assert_eq!(keymap.action_for('z'), None);
}

#[test]
fn key_for_finds_a_bound_key() {
    let keymap = Keymap::default();
    assert_eq!(keymap.key_for(Action::Quit), Some('q'));
}

#[test]
fn config_can_rebind_an_action() {
    let mut config = Config::default();
    config.keymap.insert("quit".to_string(), "x".to_string());
    let keymap = Keymap::from_config(&config);
    assert_eq!(keymap.action_for('x'), Some(Action::Quit));
}

#[test]
fn action_names_round_trip() {
    assert_eq!(Action::from_name("toggle_bell"), Some(Action::ToggleBell));
    assert_eq!(Action::from_name("volume_up"), Some(Action::VolumeUp));
    assert_eq!(Action::from_name("nope"), None);
}
