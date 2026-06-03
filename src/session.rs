use crate::audio::voice::VoiceScheduler;
use crate::audio::{self, AudioBackend};
use crate::breath::{self, Breath, Phase, PATTERNS};
use crate::cli::Cli;
use crate::config::Config;
use crate::door;
use crate::keymap::{Action, Keymap};
use crate::pack::{self, AssetKind};
use crate::palette::{self, season_for_month, time_for_hour};
use crate::paths;
use crate::render::orb::{self, OrbScene};
use crate::render::{renderer_for, Surface};
use crate::state::State;
use crate::streak;
use crate::term::{Capabilities, Env, SystemEnv};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{cursor, execute, queue};
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

const RIPPLE_TTL: f32 = 3.0;
const MILESTONE_FLASH_SECS: f32 = 1.5;
const CLOSING_PHRASES: [&str; 5] = [
    "Be at peace",
    "Stillness carries forward",
    "The path continues",
    "Return gently",
    "Carry this calm with you",
];

/// How a session ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndMode {
    OpenEnded,
    After(Duration),
    Breaths(u32),
}

pub fn end_mode(for_arg: Option<&str>, breaths: Option<u32>) -> Option<EndMode> {
    if let Some(n) = breaths {
        return Some(EndMode::Breaths(n));
    }
    match for_arg {
        None => Some(EndMode::OpenEnded),
        Some(text) => parse_duration(text).map(EndMode::After),
    }
}

pub fn should_end(mode: EndMode, elapsed: Duration, breaths: u32) -> bool {
    match mode {
        EndMode::OpenEnded => false,
        EndMode::After(limit) => elapsed >= limit,
        EndMode::Breaths(target) => breaths >= target,
    }
}

/// Parse a duration like `90s`, `5m`, or `1h30m`. A bare integer is seconds.
pub fn parse_duration(text: &str) -> Option<Duration> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    if let Ok(seconds) = text.parse::<u64>() {
        return (seconds > 0).then(|| Duration::from_secs(seconds));
    }

    let mut total = 0u64;
    let mut digits = String::new();
    for ch in text.chars() {
        if ch.is_ascii_digit() {
            digits.push(ch);
            continue;
        }
        let value: u64 = digits.parse().ok()?;
        digits.clear();
        let unit: u64 = match ch {
            'h' => 3600,
            'm' => 60,
            's' => 1,
            _ => return None,
        };
        total = total.checked_add(value.checked_mul(unit)?)?;
    }
    if !digits.is_empty() || total == 0 {
        return None;
    }
    Some(Duration::from_secs(total))
}

/// Fires each milestone (5/10/15/20/30 min) exactly once.
#[derive(Default)]
pub struct MilestoneTracker {
    fired: Vec<u64>,
}

impl MilestoneTracker {
    pub fn new() -> MilestoneTracker {
        MilestoneTracker::default()
    }

    pub fn check(&mut self, elapsed_secs: u64) -> Option<u64> {
        let mark = breath::milestone_window(elapsed_secs)?;
        if self.fired.contains(&mark) {
            return None;
        }
        self.fired.push(mark);
        Some(mark)
    }
}

pub fn reduce_motion_enabled(flag: bool, config: &Config, env: &impl Env) -> bool {
    flag || config.reduce_motion.unwrap_or(false) || Capabilities::detect(env).reduce_motion
}

/// Civil (year, month, day) from a count of days since the Unix epoch, via
/// Howard Hinnant's algorithm. Used to pick the seasonal palette.
pub fn ymd_from_unix_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (year + i64::from(month <= 2), month, day)
}

fn now_month_hour() -> (u32, u32) {
    let since = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = since.as_secs() as i64;
    let (_, month, _) = ymd_from_unix_days(secs.div_euclid(86_400));
    (month, (secs.rem_euclid(86_400) / 3600) as u32)
}

fn cycle_pattern(current: &str, delta: i32) -> breath::Pattern {
    let index = PATTERNS.iter().position(|p| p.name == current).unwrap_or(0) as i32;
    let len = PATTERNS.len() as i32;
    PATTERNS[(index + delta).rem_euclid(len) as usize]
}

/// Advance a "none → 0 → 1 → … → last → none" selection cycle.
fn next_cycle_index(current: Option<usize>, len: usize) -> Option<usize> {
    match current {
        None => (len > 0).then_some(0),
        Some(i) if i + 1 < len => Some(i + 1),
        Some(_) => None,
    }
}

/// Format a kebab-case pattern name for display: "deep-calm" -> "Deep calm".
fn title_case(name: &str) -> String {
    let spaced = name.replace('-', " ");
    let mut chars = spaced.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => spaced,
    }
}

/// Restores the terminal on every exit path, including a panic unwind.
struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> io::Result<TerminalGuard> {
        terminal::enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen, cursor::Hide)?;
        Ok(TerminalGuard)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), cursor::Show, LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}

pub fn run(cli: &Cli) -> i32 {
    let config_dir = match paths::config_dir() {
        Ok(dir) => dir,
        Err(err) => {
            eprintln!("meditate: {err}");
            return 1;
        }
    };
    let data_dir = match paths::data_dir() {
        Ok(dir) => dir,
        Err(err) => {
            eprintln!("meditate: {err}");
            return 1;
        }
    };

    if cli.breaths == Some(0) {
        eprintln!("meditate: --breaths must be at least 1");
        return 1;
    }

    let mut mode = match end_mode(cli.r#for.as_deref(), cli.breaths) {
        Some(mode) => mode,
        None => {
            eprintln!("meditate: could not understand --for value (try 5m, 90s, 1h30m)");
            return 1;
        }
    };

    let config = Config::load_or_default(&config_dir);
    let state = State::load_from(&data_dir);
    let streak_enabled = config.streak_enabled.unwrap_or(true) && !cli.no_streak;

    if !io::stdout().is_terminal() {
        eprintln!("meditate needs an interactive terminal — stdout is not a TTY.");
        return 1;
    }

    let pattern_name =
        crate::resolve_start_pattern(cli.pattern.map(|p| p.as_str()), &config, &state)
            .unwrap_or_else(|| "calm".to_string());
    if matches!(mode, EndMode::Breaths(_)) && breath::pattern_by_name(&pattern_name).is_still() {
        println!("meditate: the 'none' pattern has no breaths to count — running open-ended.");
        mode = EndMode::OpenEnded;
    }

    if streak_enabled {
        let record = streak::Streak::load_from(&data_dir);
        if record.current_streak > 0 {
            println!(
                "  {} days running · {} min total",
                record.current_streak,
                record.total_minutes()
            );
        }
    }

    // Civil day of the session's start, so a sit crossing midnight credits the
    // day it began (see streak.rs).
    let session_day = streak::today_utc();
    let _guard = match TerminalGuard::enter() {
        Ok(guard) => guard,
        Err(err) => {
            eprintln!("meditate: could not set up the terminal: {err}");
            return 1;
        }
    };

    let session = Session::start(cli, &config, &state, &data_dir, mode);
    let outcome = session.run_loop();

    drop(_guard);
    let _ = State {
        last_pattern: Some(outcome.pattern_name.clone()),
    }
    .save_to(&data_dir);

    if streak_enabled {
        let _ = streak::record_session(&data_dir, session_day, outcome.elapsed.as_secs());
    }

    print_summary(&outcome);
    0
}

struct Session {
    breath: Breath,
    renderer: Box<dyn crate::render::Renderer>,
    audio: Box<dyn AudioBackend>,
    keymap: Keymap,
    mode: EndMode,
    reduce_motion: bool,
    door_enabled: bool,
    palette: palette::Palette,
    master: f32,
    muted: bool,
    focus: bool,
    soundscapes: Vec<(String, PathBuf)>,
    soundscape_idx: Option<usize>,
    voices: Vec<(String, PathBuf)>,
    voice_idx: Option<usize>,
    voice: Option<VoiceScheduler>,
}

struct Outcome {
    elapsed: Duration,
    breaths: u32,
    pattern_name: String,
    door_enabled: bool,
}

impl Session {
    fn start(cli: &Cli, config: &Config, state: &State, data_dir: &Path, mode: EndMode) -> Session {
        let env = SystemEnv;
        let caps = Capabilities::detect(&env);
        let (month, hour) = now_month_hour();
        let palette = palette::resolve_with_pin(
            season_for_month(month),
            time_for_hour(hour),
            cli.pin_palette,
        );
        let pattern_name =
            crate::resolve_start_pattern(cli.pattern.map(|p| p.as_str()), config, state)
                .unwrap_or_else(|| "calm".to_string());

        let audio = audio::open();
        audio.bell();
        let master = config
            .master_volume
            .map(|v| f32::from(v) / 100.0)
            .unwrap_or(0.8);
        audio.set_master(master);

        Session {
            breath: Breath::new(breath::pattern_by_name(&pattern_name), Duration::ZERO),
            renderer: renderer_for(&caps),
            audio,
            keymap: Keymap::from_config(config),
            mode,
            reduce_motion: reduce_motion_enabled(cli.reduce_motion, config, &env),
            door_enabled: config.door_enabled.unwrap_or(true) && !cli.no_door,
            palette,
            master,
            muted: false,
            focus: false,
            soundscapes: pack::cached_files(data_dir, AssetKind::Soundscape),
            soundscape_idx: None,
            voices: pack::cached_files(data_dir, AssetKind::Voice),
            voice_idx: None,
            voice: None,
        }
    }

    fn run_loop(mut self) -> Outcome {
        let start = Instant::now();
        let mut last_frame = start;
        let mut ripples: Vec<f32> = Vec::new();
        let mut milestones = MilestoneTracker::new();
        let mut last_breath = 0u32;
        let mut flash_remaining = 0.0f32;
        let mut hint_until = start + Duration::from_secs(4);
        let mut message = String::new();
        let mut message_expiry = start;

        loop {
            let now = start.elapsed();
            let frame_now = Instant::now();
            let dt = frame_now.duration_since(last_frame).as_secs_f32();
            last_frame = frame_now;

            if frame_now >= message_expiry {
                message.clear();
            }

            let state = self.breath.tick(now);
            if state.breath_count > last_breath {
                ripples.push(0.0);
                last_breath = state.breath_count;
            }
            for life in ripples.iter_mut() {
                *life += dt / RIPPLE_TTL;
            }
            ripples.retain(|life| *life < 1.0);

            if milestones.check(now.as_secs()).is_some() {
                flash_remaining = MILESTONE_FLASH_SECS;
            }
            flash_remaining = (flash_remaining - dt).max(0.0);

            self.tick_voice(now.as_secs());

            if should_end(self.mode, now, state.breath_count) {
                self.audio.bell();
                break;
            }

            let hint_visible = frame_now < hint_until || !message.is_empty();
            let _ = self.draw(
                state,
                &ripples,
                flash_remaining / MILESTONE_FLASH_SECS,
                hint_visible,
                &message,
            );

            let interval = self.frame_interval(state.phase);
            if let Ok(true) = event::poll(interval) {
                if let Ok(Event::Key(key)) = event::read() {
                    if key.kind != KeyEventKind::Release {
                        hint_until = Instant::now() + Duration::from_secs(4);
                        match classify_key(&key, &self.keymap) {
                            KeyOutcome::Quit => break,
                            KeyOutcome::Act(action) => {
                                if let Some(text) = self.apply(action, now) {
                                    message = text;
                                    message_expiry = Instant::now() + Duration::from_secs(3);
                                }
                            }
                            KeyOutcome::Ignore => {}
                        }
                    }
                }
            }
        }

        self.fade_out();
        Outcome {
            elapsed: start.elapsed(),
            breaths: self.breath.breath_count(),
            pattern_name: self.breath.pattern().name.to_string(),
            door_enabled: self.door_enabled,
        }
    }

    /// Apply a control action. Returns an optional transient message to show
    /// (e.g. the missing-pack hint), which doubles as focus-mode confirmation.
    fn apply(&mut self, action: Action, now: Duration) -> Option<String> {
        match action {
            Action::NextPattern => {
                let next = cycle_pattern(self.breath.pattern().name, 1);
                self.breath.switch_to(next, now);
                self.focus.then(|| title_case(next.name))
            }
            Action::PrevPattern => {
                let prev = cycle_pattern(self.breath.pattern().name, -1);
                self.breath.switch_to(prev, now);
                self.focus.then(|| title_case(prev.name))
            }
            Action::CycleSoundscape => self.cycle_soundscape(),
            Action::CycleVoice => self.cycle_voice(),
            Action::ToggleBell => {
                self.audio.bell();
                self.focus.then(|| "Bell".to_string())
            }
            Action::Mute => {
                self.muted = !self.muted;
                self.audio.set_muted(self.muted);
                self.focus
                    .then(|| if self.muted { "Muted" } else { "Unmuted" }.to_string())
            }
            Action::VolumeUp => {
                if self.muted {
                    self.muted = false;
                    self.audio.set_muted(false);
                }
                self.master = (self.master + 0.1).min(1.0);
                self.audio.set_master(self.master);
                None
            }
            Action::VolumeDown => {
                self.master = (self.master - 0.1).max(0.0);
                self.audio.set_master(self.master);
                None
            }
            Action::Pause => {
                self.breath.toggle_pause(now);
                self.focus.then(|| {
                    if self.breath.is_paused() {
                        "Paused"
                    } else {
                        "Resumed"
                    }
                    .to_string()
                })
            }
            Action::Focus => {
                self.focus = !self.focus;
                self.focus.then(|| "Focus".to_string())
            }
            Action::Quit => None,
        }
    }

    /// Cycle the soundscape: off → first → … → last → off. Decoding happens on
    /// this thread (a brief hitch when switching a large loop); a background
    /// decode is a worthwhile follow-up.
    fn cycle_soundscape(&mut self) -> Option<String> {
        if self.soundscapes.is_empty() {
            return Some("No soundscape pack — run: meditate download soundscapes".to_string());
        }
        self.soundscape_idx = next_cycle_index(self.soundscape_idx, self.soundscapes.len());
        match self.soundscape_idx {
            None => {
                self.audio.stop_soundscape();
                Some("soundscape off".to_string())
            }
            Some(i) => {
                let (id, path) = self.soundscapes[i].clone();
                match pack::soundscape::load_samples(&path, self.audio.sample_rate()) {
                    Some(samples) => {
                        self.audio.play_soundscape(Arc::new(samples));
                        Some(title_case(&id))
                    }
                    None => Some(format!("couldn't play {id}")),
                }
            }
        }
    }

    /// Cycle the voice guide: off → first pack → … → off. Selecting a pack loads
    /// its meditation prompts into a scheduler; `tick_voice` plays them over time.
    fn cycle_voice(&mut self) -> Option<String> {
        if self.voices.is_empty() {
            return Some("No voice pack — run: meditate download voices".to_string());
        }
        self.voice_idx = next_cycle_index(self.voice_idx, self.voices.len());
        match self.voice_idx {
            None => {
                self.voice = None;
                Some("voice off".to_string())
            }
            Some(i) => {
                let (id, dir) = self.voices[i].clone();
                match pack::load_voice_prompts(&dir) {
                    Some(prompts) if !prompts.is_empty() => {
                        self.voice = Some(VoiceScheduler::new(prompts));
                        Some(title_case(&id))
                    }
                    _ => {
                        self.voice = None;
                        self.voice_idx = None;
                        Some(format!("couldn't load {id}"))
                    }
                }
            }
        }
    }

    /// Play the next due voice prompt, decoding it and ducking the soundscape
    /// beneath it. Decoding runs on the loop thread, but prompts are short so the
    /// hitch is small.
    fn tick_voice(&mut self, elapsed_secs: u64) {
        let Some(idx) = self.voice_idx else {
            return;
        };
        let prompt = match self.voice.as_mut() {
            Some(scheduler) => scheduler.next(elapsed_secs),
            None => return,
        };
        let Some(prompt) = prompt else {
            return;
        };
        let Some(safe_id) = pack::safe_component(&prompt.id) else {
            return;
        };
        let path = self.voices[idx].1.join(format!("{safe_id}.aac"));
        if let Some(samples) = pack::soundscape::load_samples(&path, self.audio.sample_rate()) {
            self.audio.play_voice(Arc::new(samples));
        }
    }

    fn frame_interval(&self, phase: Phase) -> Duration {
        if self.reduce_motion || self.breath.is_paused() {
            Duration::from_millis(200)
        } else if matches!(phase, Phase::HoldIn | Phase::HoldOut | Phase::Still) {
            Duration::from_millis(100)
        } else {
            Duration::from_millis(33)
        }
    }

    fn draw(
        &self,
        state: breath::PhaseState,
        ripples: &[f32],
        flash: f32,
        hint_visible: bool,
        message: &str,
    ) -> io::Result<()> {
        let (cols, rows) = terminal::size()?;
        if cols == 0 || rows < 2 {
            return Ok(());
        }
        let mut surface = Surface::new(
            cols as usize,
            (rows as usize - 1) * 2,
            self.palette.background,
        );
        let scene = OrbScene {
            scale: orb::scale_for(state),
            glow: orb::glow_for(state),
            ripples: ripples.to_vec(),
            milestone_flash: flash,
            palette: self.palette,
        };
        orb::paint(&mut surface, &scene);

        let mut stdout = io::stdout();
        queue!(stdout, cursor::MoveTo(0, 0))?;
        stdout.write_all(self.renderer.encode(&surface).as_bytes())?;

        queue!(
            stdout,
            cursor::MoveTo(0, rows - 1),
            Clear(ClearType::CurrentLine)
        )?;
        if !self.focus {
            write!(stdout, "{}", self.status_line(state, hint_visible, message))?;
        } else if !message.is_empty() {
            write!(stdout, "{message}")?;
        }
        stdout.flush()
    }

    fn status_line(&self, state: breath::PhaseState, hint_visible: bool, message: &str) -> String {
        let mut line = format!(
            "{}  ·  {}  ·  breath {}",
            title_case(self.breath.pattern().name),
            state.phase.label(),
            state.breath_count
        );
        if !message.is_empty() {
            line.push_str("  ·  ");
            line.push_str(message);
        } else if hint_visible {
            line.push_str("  ·  q quit · space pause · n pattern · b bell · m mute · f focus");
        }
        line
    }

    fn fade_out(&mut self) {
        for step in 0..12 {
            let scale = (0.7 - step as f32 * 0.05).max(0.05);
            let scene = OrbScene {
                scale,
                glow: 0.0,
                ripples: Vec::new(),
                milestone_flash: 0.0,
                palette: self.palette,
            };
            if let Ok((cols, rows)) = terminal::size() {
                if cols > 0 && rows >= 2 {
                    let mut surface = Surface::new(
                        cols as usize,
                        (rows as usize - 1) * 2,
                        self.palette.background,
                    );
                    orb::paint(&mut surface, &scene);
                    let mut stdout = io::stdout();
                    let _ = queue!(stdout, cursor::MoveTo(0, 0));
                    let _ = stdout.write_all(self.renderer.encode(&surface).as_bytes());
                    let _ = stdout.flush();
                }
            }
            std::thread::sleep(Duration::from_millis(40));
        }
    }
}

fn is_quit(key: &event::KeyEvent) -> bool {
    matches!(key.code, KeyCode::Esc)
        || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
}

#[derive(Debug, PartialEq, Eq)]
enum KeyOutcome {
    Quit,
    Act(Action),
    Ignore,
}

/// Decide what a keypress means: quit (the keymap's quit binding, Esc, or
/// Ctrl-C), a control action, or nothing. Pure, so the quit path is testable
/// without a terminal.
fn classify_key(key: &event::KeyEvent, keymap: &Keymap) -> KeyOutcome {
    if is_quit(key) {
        return KeyOutcome::Quit;
    }
    if let KeyCode::Char(ch) = key.code {
        match keymap.action_for(ch) {
            Some(Action::Quit) => return KeyOutcome::Quit,
            Some(action) => return KeyOutcome::Act(action),
            None => {}
        }
    }
    KeyOutcome::Ignore
}

fn print_summary(outcome: &Outcome) {
    let minutes = outcome.elapsed.as_secs() / 60;
    let seconds = outcome.elapsed.as_secs() % 60;
    let phrase = CLOSING_PHRASES[(outcome.breaths as usize) % CLOSING_PHRASES.len()];
    println!();
    println!("  {minutes}m {seconds}s · {} breaths", outcome.breaths);
    println!("  {phrase}.");
    if door::should_show(
        outcome.elapsed,
        door::DEFAULT_LONG_SESSION,
        outcome.door_enabled,
    ) {
        println!();
        println!("  {}", door::INVITATION);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycle_pattern_wraps_both_directions_and_clamps_unknown() {
        let first = PATTERNS[0].name;
        let last = PATTERNS[PATTERNS.len() - 1].name;
        assert_eq!(cycle_pattern(last, 1).name, first);
        assert_eq!(cycle_pattern(first, -1).name, last);
        assert_eq!(cycle_pattern(first, 1).name, PATTERNS[1].name);
        assert_eq!(cycle_pattern("wobble", 0).name, first);
    }

    #[test]
    fn q_esc_and_ctrl_c_all_quit() {
        let keymap = Keymap::default();
        let char_key = |c| event::KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE);

        assert_eq!(classify_key(&char_key('q'), &keymap), KeyOutcome::Quit);
        assert_eq!(
            classify_key(
                &event::KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
                &keymap
            ),
            KeyOutcome::Quit
        );
        assert_eq!(
            classify_key(
                &event::KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
                &keymap
            ),
            KeyOutcome::Quit
        );
        assert_eq!(
            classify_key(&char_key('n'), &keymap),
            KeyOutcome::Act(Action::NextPattern)
        );
        assert_eq!(classify_key(&char_key('Z'), &keymap), KeyOutcome::Ignore);
    }

    #[test]
    fn soundscape_cycle_runs_off_through_each_then_off() {
        assert_eq!(next_cycle_index(None, 2), Some(0));
        assert_eq!(next_cycle_index(Some(0), 2), Some(1));
        assert_eq!(next_cycle_index(Some(1), 2), None);
        assert_eq!(next_cycle_index(None, 0), None);
    }
}
