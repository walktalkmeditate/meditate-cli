use crate::cli::PalettePin;
use crate::render::Rgb;

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
pub fn resolve_with_pin(
    mut season: Season,
    mut time: TimeOfDay,
    pin: Option<PalettePin>,
) -> Palette {
    if let Some(pin) = pin {
        apply_pin(&mut season, &mut time, pin);
    }
    palette(season, time)
}

fn apply_pin(season: &mut Season, time: &mut TimeOfDay, pin: PalettePin) {
    match pin {
        PalettePin::Spring => *season = Season::Spring,
        PalettePin::Summer => *season = Season::Summer,
        PalettePin::Autumn => *season = Season::Autumn,
        PalettePin::Winter => *season = Season::Winter,
        PalettePin::Dawn => *time = TimeOfDay::Dawn,
        PalettePin::Day => *time = TimeOfDay::Day,
        PalettePin::Dusk => *time = TimeOfDay::Dusk,
        PalettePin::Night => *time = TimeOfDay::Night,
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
