use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorDepth {
    Truecolor,
    Ansi256,
    Ansi16,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphicsProtocol {
    Kitty,
    ITerm2,
    Sixel,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capabilities {
    pub color: ColorDepth,
    pub graphics: GraphicsProtocol,
    pub reduce_motion: bool,
}

/// Environment lookups, abstracted so capability detection is unit-testable
/// without mutating the real process environment.
pub trait Env {
    fn get(&self, key: &str) -> Option<String>;
    fn has(&self, key: &str) -> bool {
        self.get(key).is_some()
    }
}

pub struct SystemEnv;

impl Env for SystemEnv {
    fn get(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }
}

/// An in-memory environment for tests.
#[derive(Default)]
pub struct MapEnv(HashMap<String, String>);

impl MapEnv {
    pub fn new(pairs: &[(&str, &str)]) -> Self {
        MapEnv(
            pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        )
    }
}

impl Env for MapEnv {
    fn get(&self, key: &str) -> Option<String> {
        self.0.get(key).cloned()
    }
}

impl Capabilities {
    pub fn detect(env: &impl Env) -> Capabilities {
        Capabilities {
            color: detect_color(env),
            graphics: detect_graphics(env),
            reduce_motion: detect_reduce_motion(env),
        }
    }
}

fn detect_color(env: &impl Env) -> ColorDepth {
    if env.has("NO_COLOR") {
        return ColorDepth::None;
    }
    let term = env.get("TERM").unwrap_or_default();
    if term.is_empty() || term == "dumb" {
        return ColorDepth::None;
    }
    if matches!(
        env.get("COLORTERM").as_deref(),
        Some("truecolor") | Some("24bit")
    ) {
        return ColorDepth::Truecolor;
    }
    if term.contains("256color") {
        return ColorDepth::Ansi256;
    }
    ColorDepth::Ansi16
}

fn detect_graphics(env: &impl Env) -> GraphicsProtocol {
    if env.has("KITTY_WINDOW_ID") || env.get("TERM").as_deref() == Some("xterm-kitty") {
        return GraphicsProtocol::Kitty;
    }
    match env.get("TERM_PROGRAM").as_deref() {
        Some("iTerm.app") => GraphicsProtocol::ITerm2,
        Some("WezTerm") => GraphicsProtocol::Kitty,
        _ => GraphicsProtocol::None,
    }
}

fn detect_reduce_motion(env: &impl Env) -> bool {
    matches!(env.get("REDUCE_MOTION").as_deref(), Some(v) if !v.is_empty() && v != "0")
}
