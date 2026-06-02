use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const CONFIG_FILE: &str = "config.toml";

/// Hand-edited user preferences. Every field is optional; an absent file or
/// absent key falls back to a built-in default, so a zero-config launch works.
/// meditate never writes this file — that keeps user edits authoritative.
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
