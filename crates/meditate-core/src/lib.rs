//! The pure core of meditate: the breath engine, the orb scene, the half-block
//! renderers, the seasonal palette, and the title ramp — everything that is
//! free of I/O, clocks, and the terminal, so it compiles to both the native CLI
//! and `wasm32-unknown-unknown` for the web build.
//!
//! What lives here is deterministic: the breath engine is driven by a session
//! clock the caller supplies on each tick, and capability/palette inputs are
//! passed in rather than detected. The native binding (`std::env`, the real
//! clock, the terminal) stays in the `meditate` crate.

pub mod breath;
pub mod caps;
pub mod palette;
pub mod render;
pub mod title;
