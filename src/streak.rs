use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

pub const STREAK_FILE: &str = "streak.toml";

/// A session must run at least this long to earn streak credit, so an
/// open-and-quit doesn't count as a day.
pub const MIN_SESSION_SECS: u64 = 60;

/// Local, account-free practice record. A day is the civil day of a session's
/// start; a session that crosses midnight credits the day it began.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Streak {
    pub total_seconds: u64,
    pub current_streak: u32,
    pub longest_streak: u32,
    pub last_day: Option<i64>,
}

impl Streak {
    pub fn total_minutes(&self) -> u64 {
        self.total_seconds / 60
    }

    /// Fold a finished session into the record. Sessions under the minimum earn
    /// nothing. `today` is the civil day number (days since the Unix epoch) of
    /// the session's start.
    pub fn record(&mut self, today: i64, session_secs: u64) {
        if session_secs < MIN_SESSION_SECS {
            return;
        }
        self.total_seconds += session_secs;
        match self.last_day {
            Some(day) if day == today => {}
            Some(day) if today == day + 1 => self.current_streak += 1,
            // Clock moved backward (NTP correction, travel) — leave the streak intact.
            Some(day) if today < day => {}
            _ => self.current_streak = 1,
        }
        self.last_day = Some(today);
        self.longest_streak = self.longest_streak.max(self.current_streak);
    }

    pub fn path_in(dir: &Path) -> PathBuf {
        dir.join(STREAK_FILE)
    }

    /// Read the record, treating missing or corrupt files as no history so a
    /// bad file never blocks a launch.
    pub fn load_from(dir: &Path) -> Streak {
        std::fs::read_to_string(Self::path_in(dir))
            .ok()
            .and_then(|text| toml::from_str(&text).ok())
            .unwrap_or_default()
    }
}

/// Civil day number (days since the Unix epoch) for now.
pub fn today_utc() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| (since.as_secs() / 86_400) as i64)
        .unwrap_or(0)
}

/// Record a session under an exclusive file lock so two concurrent instances
/// each fold their minutes in (read-modify-write, not last-writer-wins).
pub fn record_session(dir: &Path, today: i64, session_secs: u64) -> std::io::Result<Streak> {
    if session_secs < MIN_SESSION_SECS {
        return Ok(Streak::load_from(dir));
    }
    std::fs::create_dir_all(dir)?;
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(Streak::path_in(dir))?;
    file.lock_exclusive()?;

    let result = (|| {
        let mut text = String::new();
        file.read_to_string(&mut text)?;
        let mut streak: Streak = toml::from_str(&text).unwrap_or_default();
        streak.record(today, session_secs);
        let serialized = toml::to_string_pretty(&streak).expect("streak serializes to TOML");
        file.set_len(0)?;
        file.seek(SeekFrom::Start(0))?;
        file.write_all(serialized.as_bytes())?;
        Ok(streak)
    })();

    let _ = file.unlock();
    result
}

/// Erase the local record. A missing file is already "reset".
pub fn reset(dir: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(Streak::path_in(dir)) {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}
