pub mod audio;
pub mod breath;
pub mod cli;
pub mod config;
pub mod door;
pub mod keymap;
pub mod palette;
pub mod paths;
pub mod render;
pub mod session;
pub mod state;
pub mod term;

use cli::{Cli, Command, ConfigAction};
use config::Config;
use state::State;

/// Decide which breathing pattern a session opens with.
///
/// A pattern named on the command line always wins. Otherwise, when
/// `resume_last_pattern` is enabled (the default), the last-used pattern from
/// machine state wins; failing that, the configured default is used.
pub fn resolve_start_pattern(
    cli_pattern: Option<&str>,
    config: &Config,
    state: &State,
) -> Option<String> {
    if let Some(pattern) = cli_pattern {
        return Some(pattern.to_string());
    }
    if config.resume_last_pattern() {
        if let Some(pattern) = &state.last_pattern {
            return Some(pattern.clone());
        }
    }
    config.default_pattern.clone()
}

pub fn run(cli: Cli) -> i32 {
    match &cli.command {
        Some(Command::Config { action }) => cmd_config(action.as_ref()),
        Some(Command::Download(_)) => unwired("download", "the optional sound packs"),
        Some(Command::Integration { .. }) => unwired("integration", "the workflow nudges"),
        Some(Command::Streak { .. }) => unwired("streak", "local ritual tracking"),
        None => session::run(&cli),
    }
}

fn unwired(command: &str, what: &str) -> i32 {
    eprintln!("meditate: `{command}` arrives with {what} in a later build step.");
    1
}

fn cmd_config(action: Option<&ConfigAction>) -> i32 {
    let dir = match paths::config_dir() {
        Ok(dir) => dir,
        Err(err) => {
            eprintln!("meditate: {err}");
            return 1;
        }
    };
    let path = Config::path_in(&dir);
    match action {
        Some(ConfigAction::Path) => {
            println!("{}", path.display());
            0
        }
        _ => match std::fs::read_to_string(&path) {
            Ok(text) => {
                print!("{text}");
                0
            }
            Err(_) => {
                println!("# meditate has no config yet — built-in defaults are in use.");
                println!("# create {} to customize.", path.display());
                0
            }
        },
    }
}
