use crate::config::Config;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    NextPattern,
    PrevPattern,
    CycleSoundscape,
    CycleVoice,
    ToggleBell,
    Mute,
    VolumeUp,
    VolumeDown,
    Pause,
    Focus,
    Quit,
}

impl Action {
    pub fn from_name(name: &str) -> Option<Action> {
        Some(match name {
            "next_pattern" => Action::NextPattern,
            "prev_pattern" => Action::PrevPattern,
            "cycle_soundscape" => Action::CycleSoundscape,
            "cycle_voice" => Action::CycleVoice,
            "toggle_bell" => Action::ToggleBell,
            "mute" => Action::Mute,
            "volume_up" => Action::VolumeUp,
            "volume_down" => Action::VolumeDown,
            "pause" => Action::Pause,
            "focus" => Action::Focus,
            "quit" => Action::Quit,
            _ => return None,
        })
    }
}

/// Maps key characters to actions. Starts from the built-in defaults and lets a
/// user's config rebind any of them by action name.
pub struct Keymap {
    bindings: HashMap<char, Action>,
}

impl Keymap {
    pub fn default_bindings() -> HashMap<char, Action> {
        [
            ('n', Action::NextPattern),
            ('N', Action::PrevPattern),
            ('s', Action::CycleSoundscape),
            ('v', Action::CycleVoice),
            ('b', Action::ToggleBell),
            ('m', Action::Mute),
            ('+', Action::VolumeUp),
            ('=', Action::VolumeUp),
            ('-', Action::VolumeDown),
            (' ', Action::Pause),
            ('f', Action::Focus),
            ('q', Action::Quit),
        ]
        .into_iter()
        .collect()
    }

    pub fn from_config(config: &Config) -> Keymap {
        let mut bindings = Self::default_bindings();
        for (action_name, key) in &config.keymap {
            if let (Some(action), Some(ch)) = (Action::from_name(action_name), key.chars().next()) {
                bindings.insert(ch, action);
            }
        }
        Keymap { bindings }
    }

    pub fn action_for(&self, key: char) -> Option<Action> {
        self.bindings.get(&key).copied()
    }

    pub fn key_for(&self, action: Action) -> Option<char> {
        self.bindings
            .iter()
            .find(|(_, bound)| **bound == action)
            .map(|(key, _)| *key)
    }
}

impl Default for Keymap {
    fn default() -> Keymap {
        Keymap {
            bindings: Self::default_bindings(),
        }
    }
}
