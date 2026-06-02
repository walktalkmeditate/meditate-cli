pub mod cli;
pub mod config;
pub mod paths;
pub mod state;

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
        None => start_session(&cli),
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

fn start_session(cli: &Cli) -> i32 {
    let config_dir = match paths::config_dir() {
        Ok(dir) => dir,
        Err(err) => {
            eprintln!("meditate: {err}");
            return 1;
        }
    };
    let data_dir = match paths::data_dir() {
        Ok(dir) => dir,
        Err(err) => {
            eprintln!("meditate: {err}");
            return 1;
        }
    };

    let config = Config::load_or_default(&config_dir);
    let state = State::load_from(&data_dir);
    let pattern = resolve_start_pattern(cli.pattern.map(|p| p.as_str()), &config, &state)
        .unwrap_or_else(|| "calm".to_string());

    println!("meditate is ready — starting pattern: {pattern}");
    println!("the breathing screen is wired up in a later build step.");
    0
}
