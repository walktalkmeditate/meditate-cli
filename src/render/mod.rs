//! Native render surface: the pure half-block renderers and orb scene live in
//! `meditate_core::render` and are re-exported here, so existing
//! `crate::render::*` paths are unchanged. The terminal inline-graphics
//! renderers (kitty/iTerm2), which speak escape sequences xterm.js can't, stay
//! native in [`graphics`].

pub use meditate_core::render::{
    cell_gradient, mono, orb, renderer_for, Renderer, Rgb, Surface, Tier,
};

pub mod graphics;
