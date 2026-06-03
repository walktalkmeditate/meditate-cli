use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const STATE_FILE: &str = "state.toml";

/// Machine-written session state, kept separate from user config so meditate can
/// rewrite it freely without touching hand-edited preferences. It remembers what
/// you left a session at, so the next one resumes there — unless config pins a
/// value, which always wins. Streak data lives in its own file (see `streak.rs`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub last_pattern: Option<String>,
    /// Master volume, 0–100, to match the config unit.
    pub master_volume: Option<u8>,
    pub soundscape: Option<String>,
    pub voice: Option<String>,
    pub bell: Option<String>,
}

impl State {
    pub fn path_in(dir: &Path) -> PathBuf {
        dir.join(STATE_FILE)
    }

    /// Read state, treating a missing or corrupt file as "no history". State is
    /// disposable, so a parse failure must never block a launch.
    pub fn load_from(dir: &Path) -> State {
        std::fs::read_to_string(Self::path_in(dir))
            .ok()
            .and_then(|text| toml::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save_to(&self, dir: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        let text = toml::to_string_pretty(self).expect("state serializes to TOML");
        let target = Self::path_in(dir);
        let temp = target.with_extension("toml.part");
        std::fs::write(&temp, text)?;
        std::fs::rename(&temp, &target)
    }
}
