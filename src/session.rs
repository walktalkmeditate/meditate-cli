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
use crate::render::graphics::{self, ImageRenderer};
use crate::render::orb::{self, OrbScene};
use crate::render::starfield::{self, Starfield};
use crate::render::{renderer_for, Surface};
use crate::state::State;
use crate::streak;
use crate::term::{Capabilities, Env, GraphicsProtocol, SystemEnv};
use crate::title;
use crate::wait::{self, Waiter};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{cursor, execute, queue};
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

const RIPPLE_TTL: f32 = 3.0;
const MILESTONE_FLASH_SECS: f32 = 1.5;
/// A fixed seed keeps the constellation stable within a session, so a resize
/// reflows the field rather than reshuffling it.
const STARFIELD_SEED: u64 = 0x2026_0606;
/// Clearing radius as a multiple of the orb's full-inhale body radius: stars
/// stop just past the orb so the moss glow stays clear. The orb-wins check in
/// `starfield::paint` handles the transient reach of voice rings and ripples.
const CLEARING_MARGIN: f32 = 1.05;
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

/// Resolve the effective orb appearance: a `--appearance` flag wins, else the
/// config value (an unrecognized string falls back to `Auto`), else `Auto`.
fn effective_appearance(cli: &Cli, config: &Config) -> palette::Appearance {
    if let Some(appearance) = cli.appearance {
        return appearance.into();
    }
    config
        .appearance
        .as_deref()
        .and_then(palette::Appearance::from_str_opt)
        .unwrap_or(palette::Appearance::Auto)
}

/// The breath bloom for the constellation's near tier, damped to nothing under
/// reduce-motion so the field holds its static depth without animating.
fn field_bloom(reduce_motion: bool, state: breath::PhaseState) -> starfield::Bloom {
    if reduce_motion {
        starfield::Bloom::still()
    } else {
        starfield::bloom(state.phase, state.progress)
    }
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

/// Whether to mirror the breath into the terminal title this session.
fn title_enabled(cli: &Cli, config: &Config) -> bool {
    cli.title || config.tab_title.unwrap_or(false)
}

/// Whether to draw the real graphics orb: a kitty-protocol terminal, not opted
/// out, and not inside tmux (kitty graphics need passthrough there — deferred).
fn use_graphics(cli: &Cli, config: &Config, caps: &Capabilities, env: &impl Env) -> bool {
    if cli.no_graphics || config.graphics == Some(false) || env.has("TMUX") {
        return false;
    }
    !matches!(caps.graphics, GraphicsProtocol::None)
}

/// Pushes the terminal's title onto its stack on creation and pops it on drop,
/// so the user's original tab name returns on every exit path (including panic).
struct TitleGuard;

impl TitleGuard {
    fn enter() -> TitleGuard {
        let mut out = io::stdout();
        let _ = out.write_all(title::PUSH_TITLE.as_bytes());
        let _ = out.flush();
        TitleGuard
    }
}

impl Drop for TitleGuard {
    fn drop(&mut self) {
        let mut out = io::stdout();
        let _ = out.write_all(title::POP_TITLE.as_bytes());
        let _ = out.flush();
    }
}

/// Emits a graphics renderer's teardown escape on drop, so the image is cleared
/// on every exit path — including a panic unwind, which `fade_out_graphics` (only
/// reached on a clean loop end) would miss.
struct GraphicsGuard {
    teardown: String,
}

impl GraphicsGuard {
    fn new(teardown: String) -> GraphicsGuard {
        GraphicsGuard { teardown }
    }
}

impl Drop for GraphicsGuard {
    fn drop(&mut self) {
        if !self.teardown.is_empty() {
            let mut out = io::stdout();
            let _ = out.write_all(self.teardown.as_bytes());
            let _ = out.flush();
        }
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

    let waiter = match cli.until.as_deref() {
        Some(command) => match Waiter::spawn(command) {
            Ok(waiter) => Some(waiter),
            Err(err) => {
                eprintln!("meditate: could not start `{command}`: {err}");
                return 1;
            }
        },
        None => None,
    };

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

    let title_guard = title_enabled(cli, &config).then(TitleGuard::enter);

    let mut session = Session::start(cli, &config, &state, &data_dir, mode);
    session.waiting = waiter;
    // Guarantees the graphics image is torn down even on a panic (its renderer
    // knows the right escape; iTerm2's is empty, so the guard no-ops there).
    let graphics_guard = session
        .graphics
        .as_ref()
        .map(|renderer| GraphicsGuard::new(renderer.teardown()));
    let outcome = session.run_loop();

    drop(graphics_guard);
    drop(title_guard);
    drop(_guard);
    let _ = State {
        last_pattern: Some(outcome.pattern_name.clone()),
        master_volume: Some(outcome.master_volume),
        soundscape: outcome.soundscape.clone(),
        voice: outcome.voice.clone(),
        bell: outcome.bell.clone(),
    }
    .save_to(&data_dir);

    if streak_enabled {
        let _ = streak::record_session(&data_dir, session_day, outcome.elapsed.as_secs());
    }

    if let Some(report) = &outcome.wait_report {
        wait::print_report(report);
        if let Some(message) = wait::notify_message(report) {
            let mut out = io::stdout();
            let _ = out.write_all(title::notification(&message).as_bytes());
            let _ = out.flush();
        }
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
    starfield: Option<Starfield>,
    /// Last drawn (cols, rows); a change clears the screen so a shrunk frame
    /// leaves no stale star cells behind.
    last_size: Option<(u16, u16)>,
    master: f32,
    muted: bool,
    focus: bool,
    soundscapes: Vec<(String, PathBuf)>,
    soundscape_idx: Option<usize>,
    voices: Vec<(String, PathBuf)>,
    voice_idx: Option<usize>,
    voice: Option<VoiceScheduler>,
    /// When the currently-playing voice prompt finishes — drives the orb's voice
    /// rings + core-soften while a guide speaks.
    voice_until: Option<Instant>,
    bells: Vec<(String, PathBuf)>,
    bell_idx: Option<usize>,
    bell_samples: Option<Arc<Vec<f32>>>,
    title_enabled: bool,
    last_title: String,
    waiting: Option<Waiter>,
    wait_report: Option<wait::WaitReport>,
    graphics: Option<Box<dyn ImageRenderer>>,
}

struct Outcome {
    elapsed: Duration,
    breaths: u32,
    pattern_name: String,
    door_enabled: bool,
    master_volume: u8,
    soundscape: Option<String>,
    voice: Option<String>,
    bell: Option<String>,
    wait_report: Option<wait::WaitReport>,
}

impl Session {
    fn start(cli: &Cli, config: &Config, state: &State, data_dir: &Path, mode: EndMode) -> Session {
        let env = SystemEnv;
        let caps = Capabilities::detect(&env);
        let (month, hour) = now_month_hour();
        let appearance = effective_appearance(cli, config);
        let palette = palette::resolve_appearance(
            appearance,
            season_for_month(month),
            time_for_hour(hour),
            cli.pin_palette.map(Into::into),
        );
        let constellation = appearance == palette::Appearance::Constellation;
        let pattern_name =
            crate::resolve_start_pattern(cli.pattern.map(|p| p.as_str()), config, state)
                .unwrap_or_else(|| "calm".to_string());

        let audio = audio::open();
        // Master volume: a pinned config value wins, else last session's, else 80%.
        let master = f32::from(config.master_volume.or(state.master_volume).unwrap_or(80)) / 100.0;
        audio.set_master(master);

        // Constellation forces the half-block path: glyph star cells cannot be
        // carried through the pixelated inline-graphics image.
        let graphics: Option<Box<dyn ImageRenderer>> =
            if !constellation && use_graphics(cli, config, &caps, &env) {
                match caps.graphics {
                    GraphicsProtocol::Kitty => Some(Box::new(graphics::KittyRenderer::new())),
                    GraphicsProtocol::ITerm2 => Some(Box::new(graphics::ITerm2Renderer::new())),
                    GraphicsProtocol::None => None,
                }
            } else {
                None
            };

        let mut session = Session {
            breath: Breath::new(breath::pattern_by_name(&pattern_name), Duration::ZERO),
            renderer: renderer_for(&caps),
            audio,
            keymap: Keymap::from_config(config),
            mode,
            reduce_motion: reduce_motion_enabled(cli.reduce_motion, config, &env),
            door_enabled: config.door_enabled.unwrap_or(true) && !cli.no_door,
            palette,
            starfield: constellation.then(|| Starfield::new(STARFIELD_SEED)),
            last_size: None,
            master,
            muted: false,
            focus: false,
            soundscapes: pack::cached_files(data_dir, AssetKind::Soundscape),
            soundscape_idx: None,
            voices: pack::cached_files(data_dir, AssetKind::Voice),
            voice_idx: None,
            voice: None,
            voice_until: None,
            bells: pack::cached_files(data_dir, AssetKind::Bell),
            bell_idx: None,
            bell_samples: None,
            title_enabled: title_enabled(cli, config),
            last_title: String::new(),
            waiting: None,
            wait_report: None,
            graphics,
        };
        session.restore(config, state);
        // Opening strike — uses the restored bell if one was selected, else synth.
        session.ring_current_bell();
        session
    }

    /// Restore the soundscape, voice, and bell a session should open with: a
    /// pinned config default wins, else the last session's choice. Missing or
    /// no-longer-cached packs are silently skipped.
    fn restore(&mut self, config: &Config, state: &State) {
        if let Some(id) = config
            .default_soundscape
            .clone()
            .or_else(|| state.soundscape.clone())
        {
            self.select_soundscape_by_id(&id);
        }
        if let Some(id) = config.default_voice.clone().or_else(|| state.voice.clone()) {
            self.select_voice_by_id(&id);
        }
        if let Some(id) = config.default_bell.clone().or_else(|| state.bell.clone()) {
            self.select_bell_by_id(&id);
        }
    }

    fn select_soundscape_by_id(&mut self, id: &str) {
        let Some(i) = self.soundscapes.iter().position(|(name, _)| name == id) else {
            return;
        };
        let path = self.soundscapes[i].1.clone();
        if let Some(samples) = pack::soundscape::load_samples(&path, self.audio.sample_rate()) {
            self.soundscape_idx = Some(i);
            self.audio.play_soundscape(Arc::new(samples));
        }
    }

    fn select_voice_by_id(&mut self, id: &str) {
        let Some(i) = self.voices.iter().position(|(name, _)| name == id) else {
            return;
        };
        if let Some(prompts) = pack::load_voice_prompts(&self.voices[i].1) {
            if !prompts.is_empty() {
                self.voice_idx = Some(i);
                self.voice = Some(VoiceScheduler::new(prompts, audio::voice::time_seed()));
            }
        }
    }

    fn select_bell_by_id(&mut self, id: &str) {
        let Some(i) = self.bells.iter().position(|(name, _)| name == id) else {
            return;
        };
        let path = self.bells[i].1.clone();
        if let Some(samples) = pack::soundscape::load_samples(&path, self.audio.sample_rate()) {
            self.bell_idx = Some(i);
            self.bell_samples = Some(Arc::new(samples));
        }
    }

    fn run_loop(mut self) -> Outcome {
        let start = Instant::now();
        let mut last_frame = start;
        let mut ripples: Vec<f32> = Vec::new();
        let mut milestones = MilestoneTracker::new();
        let mut last_breath = 0u32;
        let mut flash_remaining = 0.0f32;
        let mut voice_env = 0.0f32;
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
            self.update_title(state);
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

            // Ease a 0..1 envelope toward 1 while a prompt is speaking; a slow
            // sine vibrates the voice rings (~2.5s, matching iOS).
            let voice_target = if self.voice_until.is_some_and(|until| frame_now < until) {
                1.0
            } else {
                0.0
            };
            voice_env += (voice_target - voice_env) * (dt / 0.5).min(1.0);
            let voice_pulse = 0.5 + 0.5 * (now.as_secs_f32() * std::f32::consts::TAU / 2.5).sin();

            // The wrapped command finishing ends the session, just like a timer.
            let finished = self.waiting.as_mut().and_then(|w| w.poll());
            if let Some(report) = finished {
                self.wait_report = Some(report);
                self.waiting = None;
                self.ring_current_bell();
                break;
            }

            if should_end(self.mode, now, state.breath_count) {
                // A timer/breath limit ends the sit; leave any --until command
                // running (and capture its report) rather than orphaning it.
                self.finish_wait(false);
                self.ring_current_bell();
                break;
            }

            let hint_visible = frame_now < hint_until || !message.is_empty();
            let _ = self.draw(
                state,
                &ripples,
                flash_remaining / MILESTONE_FLASH_SECS,
                voice_env,
                voice_pulse,
                hint_visible,
                &message,
            );

            let interval = self.frame_interval(state.phase);
            if let Ok(true) = event::poll(interval) {
                if let Ok(Event::Key(key)) = event::read() {
                    if key.kind != KeyEventKind::Release {
                        hint_until = Instant::now() + Duration::from_secs(4);
                        match classify_key(&key, &self.keymap) {
                            KeyOutcome::Quit => {
                                // Ctrl-C kills a wrapped command; q/Esc leaves it running.
                                self.finish_wait(is_ctrl_c(&key));
                                break;
                            }
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
        let wait_report = self.wait_report.take();
        Outcome {
            elapsed: start.elapsed(),
            breaths: self.breath.breath_count(),
            pattern_name: self.breath.pattern().name.to_string(),
            door_enabled: self.door_enabled,
            master_volume: (self.master * 100.0).round() as u8,
            soundscape: self.soundscape_idx.map(|i| self.soundscapes[i].0.clone()),
            voice: self.voice_idx.map(|i| self.voices[i].0.clone()),
            bell: self.bell_idx.map(|i| self.bells[i].0.clone()),
            wait_report,
        }
    }

    /// End a wrapped-command wait on quit: Ctrl-C kills the command, q/Esc leaves
    /// it running. No-op when nothing is being waited on.
    fn finish_wait(&mut self, cancel: bool) {
        if let Some(waiter) = self.waiting.take() {
            self.wait_report = Some(if cancel {
                waiter.cancel()
            } else {
                waiter.detach()
            });
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
            Action::ToggleBell => self.cycle_bell(),
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
                        self.voice = Some(VoiceScheduler::new(prompts, audio::voice::time_seed()));
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
            let secs = samples.len() as f64 / self.audio.sample_rate().max(1) as f64;
            self.voice_until = Some(Instant::now() + Duration::from_secs_f64(secs));
            self.audio.play_voice(Arc::new(samples));
        }
    }

    /// Cycle the bell and ring the new pick so it can be auditioned: synth →
    /// each downloaded bell → synth. The synth bell is the offline default, so
    /// with no downloads `b` simply rings it every press.
    fn cycle_bell(&mut self) -> Option<String> {
        if self.bells.is_empty() {
            self.audio.bell();
            return self.focus.then(|| "Bell".to_string());
        }
        self.bell_idx = next_cycle_index(self.bell_idx, self.bells.len());
        match self.bell_idx {
            None => {
                self.bell_samples = None;
                self.audio.bell();
                Some("Bell (synth)".to_string())
            }
            Some(i) => {
                let (id, path) = self.bells[i].clone();
                match pack::soundscape::load_samples(&path, self.audio.sample_rate()) {
                    Some(samples) => {
                        let samples = Arc::new(samples);
                        self.audio.play_bell(Arc::clone(&samples));
                        self.bell_samples = Some(samples);
                        Some(title_case(&id))
                    }
                    None => {
                        self.bell_samples = None;
                        self.bell_idx = None;
                        Some(format!("couldn't play {id}"))
                    }
                }
            }
        }
    }

    /// Ring the currently selected bell — a downloaded one if picked, else the
    /// synth default. Used for the session's opening and closing strikes.
    fn ring_current_bell(&self) {
        match &self.bell_samples {
            Some(samples) => self.audio.play_bell(Arc::clone(samples)),
            None => self.audio.bell(),
        }
    }

    /// Mirror the breath into the terminal title, writing the OSC sequence only
    /// when the rendered line changes — which throttles it to a handful of
    /// updates per breath rather than once per frame.
    fn update_title(&mut self, state: breath::PhaseState) {
        if !self.title_enabled {
            return;
        }
        let next = title::breath_title(state);
        if next != self.last_title {
            let mut out = io::stdout();
            let _ = out.write_all(title::set_sequence(&next).as_bytes());
            let _ = out.flush();
            self.last_title = next;
        }
    }

    fn frame_interval(&self, phase: Phase) -> Duration {
        if self.reduce_motion || self.breath.is_paused() {
            Duration::from_millis(200)
        } else if matches!(phase, Phase::HoldIn | Phase::HoldOut | Phase::Still) {
            Duration::from_millis(100)
        } else if self.graphics.is_some() {
            // A full image transmits each frame; ~22fps keeps bandwidth in check
            // and is imperceptible on the slow-moving orb.
            Duration::from_millis(45)
        } else {
            Duration::from_millis(33)
        }
    }

    #[allow(clippy::too_many_arguments)] // render inputs for one frame
    fn draw(
        &mut self,
        state: breath::PhaseState,
        ripples: &[f32],
        flash: f32,
        voice: f32,
        voice_pulse: f32,
        hint_visible: bool,
        message: &str,
    ) -> io::Result<()> {
        let (cols, rows) = terminal::size()?;
        if cols == 0 || rows < 2 {
            return Ok(());
        }
        let resized = self.last_size != Some((cols, rows));
        self.last_size = Some((cols, rows));
        let (cols, orb_rows) = (cols as usize, rows as usize - 1);
        let scene = OrbScene {
            scale: orb::scale_for(state),
            glow: orb::glow_for(state),
            ripples: ripples.to_vec(),
            milestone_flash: flash,
            voice,
            voice_pulse,
            palette: self.palette,
            soft_edge: self.starfield.is_some(),
        };

        let mut stdout = io::stdout();
        // On a resize, wipe the screen first so a shrunk constellation frame
        // leaves no stranded star glyphs at the old edges.
        if resized && self.starfield.is_some() {
            queue!(stdout, Clear(ClearType::All))?;
        }
        queue!(stdout, cursor::MoveTo(0, 0))?;
        if let Some(kitty) = &self.graphics {
            // Paint the orb into a small art grid, then block-upscale it: crisp,
            // chunky pixels rather than a smoothly-scaled image. The half-block
            // path below is unchanged.
            let (aw, ah) = graphics::art_size(cols, orb_rows);
            let mut art = Surface::new(aw, ah, self.palette.background);
            orb::paint(&mut art, &scene);
            let (pw, ph) = graphics::surface_size(cols, orb_rows);
            let surface = graphics::pixelate(&art, pw, ph);
            stdout.write_all(kitty.frame(&surface, cols, orb_rows).as_bytes())?;
        } else {
            let mut surface = Surface::new(cols, orb_rows * 2, self.palette.background);
            orb::paint(&mut surface, &scene);
            if let Some(field) = &self.starfield {
                // Clearing just past the full-inhale orb body keeps the steady
                // field off the orb; the orb-wins check in paint() handles the
                // transient reach of voice rings and ripples.
                let base = (cols.min(orb_rows * 2) as f32 / 2.0) * 0.92;
                let stars = field.cells(cols, orb_rows * 2, base * CLEARING_MARGIN);
                let bloom = field_bloom(self.reduce_motion, state);
                starfield::paint(&mut surface, &stars, bloom, self.palette.background);
            }
            stdout.write_all(self.renderer.encode(&surface).as_bytes())?;
        }

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
        if let Some(waiter) = &self.waiting {
            line.push_str("  ·  ⏳ ");
            line.push_str(&truncate_command(waiter.command()));
        }
        if !message.is_empty() {
            line.push_str("  ·  ");
            line.push_str(message);
        } else if hint_visible {
            line.push_str("  ·  q quit · space pause · n pattern · b bell · m mute · f focus");
        }
        line
    }

    fn fade_out(&mut self) {
        if self.graphics.is_some() {
            self.fade_out_graphics();
            return;
        }
        for step in 0..12 {
            let scale = (0.7 - step as f32 * 0.05).max(0.05);
            let scene = OrbScene {
                scale,
                glow: 0.0,
                ripples: Vec::new(),
                milestone_flash: 0.0,
                voice: 0.0,
                voice_pulse: 0.0,
                palette: self.palette,
                soft_edge: self.starfield.is_some(),
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

    /// The graphics-orb fade: shrink the image, then delete it so nothing lingers.
    fn fade_out_graphics(&mut self) {
        let Some(kitty) = &self.graphics else {
            return;
        };
        if let Ok((cols, rows)) = terminal::size() {
            if cols > 0 && rows >= 2 {
                let (cols, orb_rows) = (cols as usize, rows as usize - 1);
                let (pw, ph) = graphics::surface_size(cols, orb_rows);
                let (aw, ah) = graphics::art_size(cols, orb_rows);
                for step in 0..12 {
                    let scale = (0.7 - step as f32 * 0.05).max(0.05);
                    let scene = OrbScene {
                        scale,
                        glow: 0.0,
                        ripples: Vec::new(),
                        milestone_flash: 0.0,
                        voice: 0.0,
                        voice_pulse: 0.0,
                        palette: self.palette,
                        soft_edge: false,
                    };
                    let mut art = Surface::new(aw, ah, self.palette.background);
                    orb::paint(&mut art, &scene);
                    let surface = graphics::pixelate(&art, pw, ph);
                    let mut stdout = io::stdout();
                    let _ = queue!(stdout, cursor::MoveTo(0, 0));
                    let _ = stdout.write_all(kitty.frame(&surface, cols, orb_rows).as_bytes());
                    let _ = stdout.flush();
                    std::thread::sleep(Duration::from_millis(40));
                }
            }
        }
        // The image is deleted by the GraphicsGuard on exit, so it's cleared on a
        // panic too — not only here on a clean end.
    }
}

fn is_quit(key: &event::KeyEvent) -> bool {
    matches!(key.code, KeyCode::Esc)
        || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
}

fn is_ctrl_c(key: &event::KeyEvent) -> bool {
    key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL)
}

/// Shorten a command for the status bar so a long pipeline doesn't overflow, and
/// drop any control bytes so the command can't inject escapes into the HUD.
fn truncate_command(command: &str) -> String {
    const MAX: usize = 28;
    let command: String = command.chars().filter(|c| !c.is_control()).collect();
    let command = command.trim();
    if command.chars().count() > MAX {
        let head: String = command.chars().take(MAX - 1).collect();
        format!("{head}…")
    } else {
        command.to_string()
    }
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

    #[test]
    fn title_enabled_via_flag_or_config() {
        use clap::Parser;
        let plain = Cli::try_parse_from(["meditate"]).unwrap();
        let flagged = Cli::try_parse_from(["meditate", "--title"]).unwrap();
        assert!(title_enabled(&flagged, &Config::default()));
        assert!(!title_enabled(&plain, &Config::default()));
        let cfg = Config {
            tab_title: Some(true),
            ..Config::default()
        };
        assert!(title_enabled(&plain, &cfg));
    }

    #[test]
    fn appearance_flag_overrides_config_else_falls_back_to_auto() {
        use clap::Parser;
        use meditate_core::palette::Appearance as Core;

        let plain = Cli::try_parse_from(["meditate"]).unwrap();
        assert_eq!(effective_appearance(&plain, &Config::default()), Core::Auto);

        let dark_flag = Cli::try_parse_from(["meditate", "--appearance", "dark"]).unwrap();
        assert_eq!(
            effective_appearance(&dark_flag, &Config::default()),
            Core::Dark
        );

        // The CLI flag wins over a config value.
        let cfg_constellation = Config {
            appearance: Some("constellation".to_string()),
            ..Config::default()
        };
        assert_eq!(
            effective_appearance(&dark_flag, &cfg_constellation),
            Core::Dark
        );
        assert_eq!(
            effective_appearance(&plain, &cfg_constellation),
            Core::Constellation
        );

        // An unrecognized config value falls back to auto rather than erroring.
        let cfg_bad = Config {
            appearance: Some("nope".to_string()),
            ..Config::default()
        };
        assert_eq!(effective_appearance(&plain, &cfg_bad), Core::Auto);
    }

    #[test]
    fn reduce_motion_damps_the_constellation_bloom() {
        let state = breath::PhaseState {
            phase: breath::Phase::Exhale,
            progress: 1.0,
            breath_count: 0,
        };
        let calm = field_bloom(true, state);
        assert_eq!(calm.gain, 0.0);
        assert_eq!(calm.offset, 0.0);
        assert!(field_bloom(false, state).gain > 0.0);
    }

    #[test]
    fn use_graphics_needs_a_capable_terminal_and_no_opt_out() {
        use crate::term::{ColorDepth, MapEnv};
        use clap::Parser;
        let cli = Cli::try_parse_from(["meditate"]).unwrap();
        let cfg = Config::default();
        let kitty = Capabilities {
            color: ColorDepth::Truecolor,
            graphics: GraphicsProtocol::Kitty,
            reduce_motion: false,
        };
        let no_env = MapEnv::new(&[]);

        assert!(use_graphics(&cli, &cfg, &kitty, &no_env));
        assert!(!use_graphics(
            &cli,
            &cfg,
            &Capabilities {
                graphics: GraphicsProtocol::None,
                ..kitty
            },
            &no_env
        ));
        assert!(!use_graphics(
            &cli,
            &cfg,
            &kitty,
            &MapEnv::new(&[("TMUX", "1")])
        ));
        let no_gfx = Cli::try_parse_from(["meditate", "--no-graphics"]).unwrap();
        assert!(!use_graphics(&no_gfx, &cfg, &kitty, &no_env));
        let cfg_off = Config {
            graphics: Some(false),
            ..Config::default()
        };
        assert!(!use_graphics(&cli, &cfg_off, &kitty, &no_env));
    }

    #[test]
    fn truncate_command_shortens_and_strips_controls() {
        assert_eq!(truncate_command("ls -la"), "ls -la");
        let long = truncate_command("a-really-long-command-that-keeps-going-and-going");
        assert!(long.ends_with('…'));
        assert_eq!(long.chars().count(), 28);
        assert_eq!(truncate_command("a\x1b]0;x\x07b"), "a]0;xb");
    }
}
