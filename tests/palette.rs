use meditate::cli::PalettePin;
use meditate::palette::{
    palette, resolve_with_pin, season_for_month, time_for_hour, Season, TimeOfDay,
};

#[test]
fn months_map_to_seasons() {
    assert_eq!(season_for_month(4), Season::Spring);
    assert_eq!(season_for_month(7), Season::Summer);
    assert_eq!(season_for_month(10), Season::Autumn);
    assert_eq!(season_for_month(1), Season::Winter);
    assert_eq!(season_for_month(12), Season::Winter);
}

#[test]
fn hours_map_to_time_of_day() {
    assert_eq!(time_for_hour(6), TimeOfDay::Dawn);
    assert_eq!(time_for_hour(12), TimeOfDay::Day);
    assert_eq!(time_for_hour(18), TimeOfDay::Dusk);
    assert_eq!(time_for_hour(23), TimeOfDay::Night);
    assert_eq!(time_for_hour(3), TimeOfDay::Night);
}

#[test]
fn seasons_and_times_produce_distinct_palettes() {
    assert_ne!(
        palette(Season::Spring, TimeOfDay::Day),
        palette(Season::Winter, TimeOfDay::Day)
    );
    assert_ne!(
        palette(Season::Summer, TimeOfDay::Dawn),
        palette(Season::Summer, TimeOfDay::Night)
    );
}

#[test]
fn pin_overrides_season() {
    let pinned = resolve_with_pin(Season::Winter, TimeOfDay::Night, Some(PalettePin::Spring));
    assert_eq!(pinned, palette(Season::Spring, TimeOfDay::Night));
}

#[test]
fn pin_overrides_time_of_day() {
    let pinned = resolve_with_pin(Season::Autumn, TimeOfDay::Night, Some(PalettePin::Day));
    assert_eq!(pinned, palette(Season::Autumn, TimeOfDay::Day));
}

#[test]
fn no_pin_keeps_live_values() {
    let live = resolve_with_pin(Season::Summer, TimeOfDay::Dusk, None);
    assert_eq!(live, palette(Season::Summer, TimeOfDay::Dusk));
}
