use meditate::render::{renderer_for, Rgb, Surface, Tier};
use meditate::term::{Capabilities, ColorDepth, GraphicsProtocol, MapEnv};

#[test]
fn detects_truecolor_and_kitty() {
    let env = MapEnv::new(&[
        ("TERM", "xterm-kitty"),
        ("COLORTERM", "truecolor"),
        ("KITTY_WINDOW_ID", "1"),
    ]);
    let caps = Capabilities::detect(&env);
    assert_eq!(caps.color, ColorDepth::Truecolor);
    assert_eq!(caps.graphics, GraphicsProtocol::Kitty);
    assert!(!caps.reduce_motion);
}

#[test]
fn detects_256_color_and_iterm() {
    let env = MapEnv::new(&[("TERM", "xterm-256color"), ("TERM_PROGRAM", "iTerm.app")]);
    let caps = Capabilities::detect(&env);
    assert_eq!(caps.color, ColorDepth::Ansi256);
    assert_eq!(caps.graphics, GraphicsProtocol::ITerm2);
}

#[test]
fn no_color_and_dumb_terminals_drop_to_none() {
    let no_color = Capabilities::detect(&MapEnv::new(&[
        ("TERM", "xterm-256color"),
        ("NO_COLOR", "1"),
    ]));
    assert_eq!(no_color.color, ColorDepth::None);

    let dumb = Capabilities::detect(&MapEnv::new(&[("TERM", "dumb")]));
    assert_eq!(dumb.color, ColorDepth::None);

    let unset = Capabilities::detect(&MapEnv::new(&[]));
    assert_eq!(unset.color, ColorDepth::None);
}

#[test]
fn reduce_motion_env_is_honored() {
    assert!(
        Capabilities::detect(&MapEnv::new(&[("TERM", "xterm"), ("REDUCE_MOTION", "1")]))
            .reduce_motion
    );
    assert!(
        !Capabilities::detect(&MapEnv::new(&[("TERM", "xterm"), ("REDUCE_MOTION", "0")]))
            .reduce_motion
    );
}

#[test]
fn tier_selection_follows_color_depth() {
    let caps = |color| Capabilities {
        color,
        graphics: GraphicsProtocol::None,
        reduce_motion: false,
    };
    assert_eq!(
        Tier::select(&caps(ColorDepth::Truecolor)),
        Tier::CellGradient
    );
    assert_eq!(Tier::select(&caps(ColorDepth::Ansi256)), Tier::CellGradient);
    assert_eq!(Tier::select(&caps(ColorDepth::Ansi16)), Tier::Mono);
    assert_eq!(Tier::select(&caps(ColorDepth::None)), Tier::Mono);
}

#[test]
fn cell_gradient_emits_truecolor_half_blocks() {
    let caps = Capabilities {
        color: ColorDepth::Truecolor,
        graphics: GraphicsProtocol::None,
        reduce_motion: false,
    };
    let mut surface = Surface::new(1, 2, Rgb::BLACK);
    surface.set(0, 0, Rgb::new(10, 20, 30));
    surface.set(0, 1, Rgb::new(40, 50, 60));

    let frame = renderer_for(&caps).encode(&surface);
    assert!(frame.contains('▀'));
    assert!(frame.contains("\x1b[38;2;10;20;30m"));
    assert!(frame.contains("\x1b[48;2;40;50;60m"));
}

#[test]
fn ansi256_path_uses_indexed_escapes() {
    let caps = Capabilities {
        color: ColorDepth::Ansi256,
        graphics: GraphicsProtocol::None,
        reduce_motion: false,
    };
    let surface = Surface::new(1, 2, Rgb::new(255, 255, 255));
    let frame = renderer_for(&caps).encode(&surface);
    assert!(frame.contains("\x1b[38;5;"));
    assert!(!frame.contains("38;2;"));
}

#[test]
fn mono_maps_brightness_to_block_ramp() {
    let caps = Capabilities {
        color: ColorDepth::None,
        graphics: GraphicsProtocol::None,
        reduce_motion: false,
    };
    let renderer = renderer_for(&caps);

    let bright = {
        let mut s = Surface::new(1, 2, Rgb::new(255, 255, 255));
        s.fill(Rgb::new(255, 255, 255));
        s
    };
    assert!(renderer.encode(&bright).contains('█'));

    let dark = Surface::new(1, 2, Rgb::BLACK);
    assert_eq!(renderer.encode(&dark), " ");
}

#[test]
fn cell_gradient_produces_one_row_per_two_pixel_rows() {
    let caps = Capabilities {
        color: ColorDepth::Truecolor,
        graphics: GraphicsProtocol::None,
        reduce_motion: false,
    };
    let surface = Surface::new(3, 6, Rgb::BLACK);
    let frame = renderer_for(&caps).encode(&surface);
    assert_eq!(frame.matches("\r\n").count(), 2);
    assert_eq!(frame.matches('▀').count(), 9);
}
