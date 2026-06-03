use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Parser, Debug)]
#[command(name = "meditate", version, about = "A terminal breathing companion.")]
pub struct Cli {
    /// Breathing pattern to begin with (defaults to your last-used or configured pattern).
    pub pattern: Option<PatternName>,

    /// Run for a fixed duration (e.g. 5m, 90s) instead of open-ended.
    #[arg(long, value_name = "DURATION")]
    pub r#for: Option<String>,

    /// End after a fixed number of breaths.
    #[arg(long, value_name = "N")]
    pub breaths: Option<u32>,

    /// Don't record this session in your local streak.
    #[arg(long)]
    pub no_streak: bool,

    /// Don't show the Pilgrim invitation when a long session ends.
    #[arg(long)]
    pub no_door: bool,

    /// Pin the palette instead of letting it shift with season and time of day.
    #[arg(long, value_name = "WHEN")]
    pub pin_palette: Option<PalettePin>,

    /// Slower, calmer motion (also honored via config or the REDUCE_MOTION env var).
    #[arg(long)]
    pub reduce_motion: bool,

    /// Mirror the breathing into the terminal tab/window title (also: tab_title in config).
    #[arg(long)]
    pub title: bool,

    /// Breathe until this shell command finishes, then ring + notify (e.g. --until "cargo build").
    #[arg(long, value_name = "COMMAND")]
    pub until: Option<String>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum PatternName {
    Calm,
    Equal,
    Relaxing,
    Box,
    Coherent,
    DeepCalm,
    None,
}

impl PatternName {
    pub fn as_str(self) -> &'static str {
        match self {
            PatternName::Calm => "calm",
            PatternName::Equal => "equal",
            PatternName::Relaxing => "relaxing",
            PatternName::Box => "box",
            PatternName::Coherent => "coherent",
            PatternName::DeepCalm => "deep-calm",
            PatternName::None => "none",
        }
    }
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum PalettePin {
    Spring,
    Summer,
    Autumn,
    Winter,
    Dawn,
    Day,
    Dusk,
    Night,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Download optional sound packs (soundscapes, voices, bells).
    Download(DownloadArgs),

    /// Show your config file or its location.
    Config {
        #[command(subcommand)]
        action: Option<ConfigAction>,
    },

    /// Install or remove the shell/git/tmux breathe nudges.
    Integration {
        #[command(subcommand)]
        action: IntegrationAction,
    },

    /// Show or reset your local streak.
    Streak {
        #[command(subcommand)]
        action: Option<StreakAction>,
    },
}

#[derive(Args, Debug)]
pub struct DownloadArgs {
    /// Pack to download (e.g. soundscapes, voices, bells). Omit to list available packs.
    pub pack: Option<String>,
}

#[derive(Subcommand, Debug, PartialEq, Eq)]
pub enum ConfigAction {
    /// Print the path to your config file.
    Path,
    /// Print your current config (or the default template if you have none).
    Show,
    /// Write a fully-commented config file with every option and its default.
    #[command(visible_alias = "generate")]
    Init {
        /// Overwrite an existing config file.
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand, Debug, PartialEq, Eq)]
pub enum IntegrationAction {
    /// Add the breathe nudges to your shell, git, and tmux.
    Install,
    /// Remove the breathe nudges.
    Uninstall,
}

#[derive(Subcommand, Debug, PartialEq, Eq)]
pub enum StreakAction {
    /// Show your current streak and total minutes.
    Show,
    /// Erase your local streak history.
    Reset,
}
