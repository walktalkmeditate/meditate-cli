//! The native binding to the real terminal environment.
//!
//! The capability data and detection logic now live in `meditate_core::caps`
//! (pure, wasm-buildable, driven by the [`Env`] abstraction). This module keeps
//! only [`SystemEnv`] — the one piece that reads the real process environment —
//! and re-exports the core types so existing `crate::term::*` paths are unchanged.

pub use meditate_core::caps::{Capabilities, ColorDepth, Env, GraphicsProtocol, MapEnv};

/// The real process environment, reading `std::env`. The detection logic it
/// feeds is in the pure core; this is the only env-reading binding.
pub struct SystemEnv;

impl Env for SystemEnv {
    fn get(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }
}
