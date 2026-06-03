use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const CONFIG_FILE: &str = "config.toml";

/// Hand-edited user preferences. Every field is optional; an absent file or
/// absent key falls back to a built-in default, so a zero-config launch works.
/// meditate only writes this file when you run `config init`; it never rewrites
/// your edits during a session (that state lives in `state.toml`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub default_pattern: Option<String>,
    pub default_soundscape: Option<String>,
    pub default_voice: Option<String>,
    pub default_bell: Option<String>,
    pub master_volume: Option<u8>,
    pub palette: Option<String>,
    pub resume_last_pattern: Option<bool>,
    pub streak_enabled: Option<bool>,
    pub door_enabled: Option<bool>,
    pub nudges_enabled: Option<bool>,
    pub reduce_motion: Option<bool>,
    pub tab_title: Option<bool>,
    pub keymap: BTreeMap<String, String>,
}

#[derive(Debug)]
pub enum ConfigError {
    Read(std::io::Error),
    Parse(toml::de::Error),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Read(e) => write!(f, "could not read config: {e}"),
            ConfigError::Parse(e) => write!(f, "could not parse config: {e}"),
        }
    }
}

impl std::error::Error for ConfigError {}

impl Config {
    pub fn path_in(dir: &Path) -> PathBuf {
        dir.join(CONFIG_FILE)
    }

    pub fn load_from(dir: &Path) -> Result<Config, ConfigError> {
        match std::fs::read_to_string(Self::path_in(dir)) {
            Ok(text) => toml::from_str(&text).map_err(ConfigError::Parse),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
            Err(e) => Err(ConfigError::Read(e)),
        }
    }

    /// Load the config, or fall back to defaults after warning. A broken config
    /// must never block the breathing screen from opening.
    pub fn load_or_default(dir: &Path) -> Config {
        match Self::load_from(dir) {
            Ok(config) => config,
            Err(err) => {
                eprintln!("meditate: ignoring config ({err}); using defaults");
                Config::default()
            }
        }
    }

    pub fn save_to(&self, dir: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        let text = toml::to_string_pretty(self).expect("config serializes to TOML");
        std::fs::write(Self::path_in(dir), text)
    }

    pub fn resume_last_pattern(&self) -> bool {
        self.resume_last_pattern.unwrap_or(true)
    }
}

/// A fully-commented config covering every supported option at its default.
/// Written by `meditate config init`; shown by `meditate config` when no file
/// exists yet. Every line is commented, so loading it yields `Config::default()`
/// until the user uncomments something.
pub fn default_template() -> &'static str {
    "# meditate configuration
#
# Every setting is optional — without this file meditate still runs with sane
# defaults. Uncomment a line to override one. Anything you set here takes
# priority over what a session remembers from last time.

# ── Session ──────────────────────────────────────────────────────────────────

# Breathing pattern to start with.
# One of: calm  equal  relaxing  box  coherent  deep-calm  none
# default_pattern = \"calm\"

# When default_pattern is not set, resume the pattern you used last time.
# resume_last_pattern = true

# Master volume, 0–100.
# master_volume = 80

# Slower, calmer motion (also: --reduce-motion, or the REDUCE_MOTION env var).
# reduce_motion = false

# Mirror the breathing into the terminal tab/window title — a block that rises
# and falls with the breath, so an inactive tab still paces you (also: --title).
# In tmux, enable it with:  set -g set-titles on
# tab_title = false

# ── Sound packs ──────────────────────────────────────────────────────────────
# Download packs first, e.g.  meditate download soundscapes
# Set a default to start it on launch; otherwise meditate remembers your last
# choice. Use the pack id shown by `meditate download`.

# default_soundscape = \"forest\"
# default_voice      = \"breeze\"
# default_bell       = \"echo-chime\"

# ── Streak & door ────────────────────────────────────────────────────────────

# Count sessions toward your local streak.
# streak_enabled = true

# Show the Pilgrim invitation after a long session.
# door_enabled = true

# ── Key bindings ─────────────────────────────────────────────────────────────
# Rebind any action to a single key. Defaults shown.
# [keymap]
# next_pattern = \"n\"
# prev_pattern = \"N\"
# cycle_soundscape = \"s\"
# cycle_voice = \"v\"
# toggle_bell = \"b\"
# mute = \"m\"
# volume_up = \"+\"
# volume_down = \"-\"
# pause = \" \"
# focus = \"f\"
# quit = \"q\"
"
}
