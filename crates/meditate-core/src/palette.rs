use crate::render::Rgb;

/// A clap-free palette override, mirroring the CLI's `--pin-palette` choices.
/// Kept here (rather than reusing the CLI's clap `ValueEnum`) so the core stays
/// dependency-free; the CLI converts its enum into this via `From`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pin {
    Spring,
    Summer,
    Autumn,
    Winter,
    Dawn,
    Day,
    Dusk,
    Night,
}

/// The orb's appearance mode. `Auto` keeps the season/time-driven palette;
/// `Dark` is a fixed dark palette; `Constellation` is a self-contained
/// moss-on-deep-space look that (in later stages) carries a starfield.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Appearance {
    #[default]
    Auto,
    Dark,
    Constellation,
}

impl Appearance {
    /// Parse a config string (case-insensitive). Unknown values return `None`
    /// so callers can fall back to `Auto` rather than erroring on a typo.
    pub fn from_str_opt(s: &str) -> Option<Appearance> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(Appearance::Auto),
            "dark" => Some(Appearance::Dark),
            "constellation" => Some(Appearance::Constellation),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Season {
    Spring,
    Summer,
    Autumn,
    Winter,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeOfDay {
    Dawn,
    Day,
    Dusk,
    Night,
}

/// The four colors the orb is painted from. Derived from a base moss tone,
/// shifted by season and time of day. This is a new synthesis — Pilgrim's
/// seasonal system never touched the meditation orb (which is fixed moss there).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    pub core: Rgb,
    pub edge: Rgb,
    pub background: Rgb,
    pub ripple: Rgb,
}

const MOSS: Rgb = Rgb::new(96, 138, 102);

pub fn season_for_month(month: u32) -> Season {
    match month {
        3..=5 => Season::Spring,
        6..=8 => Season::Summer,
        9..=11 => Season::Autumn,
        _ => Season::Winter,
    }
}

pub fn time_for_hour(hour: u32) -> TimeOfDay {
    match hour {
        5..=8 => TimeOfDay::Dawn,
        9..=16 => TimeOfDay::Day,
        17..=20 => TimeOfDay::Dusk,
        _ => TimeOfDay::Night,
    }
}

pub fn palette(season: Season, time: TimeOfDay) -> Palette {
    let core = time_tint(season_tint(MOSS, season), time);
    Palette {
        core,
        edge: scale_rgb(core, 0.35),
        background: time_background(time),
        ripple: lighten(core, 0.3),
    }
}

/// Resolve the live palette, applying an optional `--pin-palette` override that
/// fixes either the season or the time of day.
pub fn resolve_with_pin(mut season: Season, mut time: TimeOfDay, pin: Option<Pin>) -> Palette {
    if let Some(pin) = pin {
        apply_pin(&mut season, &mut time, pin);
    }
    palette(season, time)
}

/// Resolve the palette for a chosen appearance. `Auto` keeps the live
/// season/time palette (honoring an optional `--pin-palette`); `Dark` and
/// `Constellation` are fixed and ignore both season/time and any pin.
pub fn resolve_appearance(
    appearance: Appearance,
    season: Season,
    time: TimeOfDay,
    pin: Option<Pin>,
) -> Palette {
    match appearance {
        Appearance::Auto => resolve_with_pin(season, time, pin),
        Appearance::Dark => dark(),
        Appearance::Constellation => constellation(),
    }
}

/// A fixed dark palette: steady moss on a deep neutral background, with no
/// seasonal or time-of-day shift.
fn dark() -> Palette {
    fixed_palette(Rgb::new(10, 12, 16))
}

/// The Stage 1 constellation orb palette: moss on a deep-indigo background,
/// matching Pilgrim iOS's Constellation canvas. The starfield is added in later
/// stages, where seasonal tinting arrives too. Public so the WASM facade can
/// switch the web orb to it (the indigo background matches the canvas cosmos, so
/// the orb's soft edge reads as glow rather than a dark fringe).
///
/// The background `#0a0a12` must stay in sync with `BASE_BG` in
/// `web/src/constellation.ts`.
pub fn constellation() -> Palette {
    fixed_palette(Rgb::new(10, 10, 18))
}

/// A fixed moss palette over `background`, with no season/time shift. Shared by
/// `dark` and `constellation`; the two diverge once Stage 3 gives constellation
/// its own seasonal tinting.
fn fixed_palette(background: Rgb) -> Palette {
    Palette {
        core: MOSS,
        edge: scale_rgb(MOSS, 0.35),
        background,
        ripple: lighten(MOSS, 0.3),
    }
}

fn apply_pin(season: &mut Season, time: &mut TimeOfDay, pin: Pin) {
    match pin {
        Pin::Spring => *season = Season::Spring,
        Pin::Summer => *season = Season::Summer,
        Pin::Autumn => *season = Season::Autumn,
        Pin::Winter => *season = Season::Winter,
        Pin::Dawn => *time = TimeOfDay::Dawn,
        Pin::Day => *time = TimeOfDay::Day,
        Pin::Dusk => *time = TimeOfDay::Dusk,
        Pin::Night => *time = TimeOfDay::Night,
    }
}

fn season_tint(c: Rgb, season: Season) -> Rgb {
    match season {
        Season::Spring => shift(c, 6, 14, -4),
        Season::Summer => shift(c, -6, 10, -8),
        Season::Autumn => shift(c, 30, -6, -18),
        Season::Winter => shift(c, -8, -4, 14),
    }
}

fn time_tint(c: Rgb, time: TimeOfDay) -> Rgb {
    match time {
        TimeOfDay::Dawn => shift(c, 18, 6, 2),
        TimeOfDay::Day => c,
        TimeOfDay::Dusk => shift(c, 26, 0, -6),
        TimeOfDay::Night => scale_rgb(c, 0.8),
    }
}

fn time_background(time: TimeOfDay) -> Rgb {
    match time {
        TimeOfDay::Dawn => Rgb::new(20, 20, 26),
        TimeOfDay::Day => Rgb::new(18, 20, 22),
        TimeOfDay::Dusk => Rgb::new(24, 18, 20),
        TimeOfDay::Night => Rgb::new(10, 12, 16),
    }
}

fn shift(c: Rgb, dr: i16, dg: i16, db: i16) -> Rgb {
    let clamp = |v: u8, d: i16| (v as i16 + d).clamp(0, 255) as u8;
    Rgb::new(clamp(c.r, dr), clamp(c.g, dg), clamp(c.b, db))
}

fn scale_rgb(c: Rgb, factor: f32) -> Rgb {
    let apply = |v: u8| (v as f32 * factor).round().clamp(0.0, 255.0) as u8;
    Rgb::new(apply(c.r), apply(c.g), apply(c.b))
}

fn lighten(c: Rgb, t: f32) -> Rgb {
    Rgb::lerp(c, Rgb::new(255, 255, 255), t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_matches_resolve_with_pin() {
        let (season, time) = (Season::Summer, TimeOfDay::Dusk);
        assert_eq!(
            resolve_appearance(Appearance::Auto, season, time, None),
            resolve_with_pin(season, time, None)
        );
    }

    #[test]
    fn dark_is_fixed_across_season_and_time() {
        let a = resolve_appearance(Appearance::Dark, Season::Summer, TimeOfDay::Day, None);
        let b = resolve_appearance(Appearance::Dark, Season::Winter, TimeOfDay::Night, None);
        assert_eq!(a, b);
        assert_eq!(a, dark());
    }

    #[test]
    fn dark_ignores_pin() {
        let pinned = resolve_appearance(
            Appearance::Dark,
            Season::Spring,
            TimeOfDay::Dawn,
            Some(Pin::Autumn),
        );
        assert_eq!(pinned, dark());
    }

    #[test]
    fn constellation_is_fixed_and_self_contained() {
        let a = resolve_appearance(
            Appearance::Constellation,
            Season::Spring,
            TimeOfDay::Dawn,
            None,
        );
        let b = resolve_appearance(
            Appearance::Constellation,
            Season::Autumn,
            TimeOfDay::Night,
            Some(Pin::Spring),
        );
        assert_eq!(a, b);
        assert_eq!(a, constellation());
    }

    #[test]
    fn from_str_opt_parses_case_insensitively() {
        assert_eq!(Appearance::from_str_opt("dark"), Some(Appearance::Dark));
        assert_eq!(Appearance::from_str_opt("  DARK "), Some(Appearance::Dark));
        assert_eq!(
            Appearance::from_str_opt("constellation"),
            Some(Appearance::Constellation)
        );
        assert_eq!(Appearance::from_str_opt("auto"), Some(Appearance::Auto));
        assert_eq!(Appearance::from_str_opt("bogus"), None);
    }
}
