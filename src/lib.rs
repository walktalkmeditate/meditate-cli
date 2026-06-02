pub mod audio;
pub mod breath;
pub mod cli;
pub mod config;
pub mod door;
pub mod integration;
pub mod keymap;
pub mod pack;
pub mod palette;
pub mod paths;
pub mod render;
pub mod session;
pub mod state;
pub mod streak;
pub mod term;

use cli::{Cli, Command, ConfigAction, DownloadArgs, IntegrationAction, StreakAction};
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
        Some(Command::Download(args)) => cmd_download(args),
        Some(Command::Integration { action }) => cmd_integration(action),
        Some(Command::Streak { action }) => cmd_streak(action.as_ref()),
        None => session::run(&cli),
    }
}

fn cmd_streak(action: Option<&StreakAction>) -> i32 {
    let dir = match paths::data_dir() {
        Ok(dir) => dir,
        Err(err) => {
            eprintln!("meditate: {err}");
            return 1;
        }
    };
    match action {
        Some(StreakAction::Reset) => match streak::reset(&dir) {
            Ok(()) => {
                println!("Your local streak has been cleared.");
                0
            }
            Err(err) => {
                eprintln!("meditate: could not clear streak: {err}");
                1
            }
        },
        _ => {
            let record = streak::Streak::load_from(&dir);
            println!(
                "  {} min total · {}-day streak · longest {} days",
                record.total_minutes(),
                record.current_streak,
                record.longest_streak
            );
            0
        }
    }
}

fn cmd_download(args: &DownloadArgs) -> i32 {
    let kind = match &args.pack {
        None => {
            println!("Optional packs: soundscapes, voices, bells.");
            println!("Download one with: meditate download soundscapes");
            return 0;
        }
        Some(name) => match pack::AssetKind::from_arg(name) {
            Some(kind) => kind,
            None => {
                eprintln!("meditate: unknown pack '{name}' (try soundscapes, voices, or bells)");
                return 1;
            }
        },
    };

    #[cfg(not(feature = "download"))]
    {
        let _ = kind;
        eprintln!(
            "meditate: this build can't download packs — use the released binary \
             or rebuild with `--features download`."
        );
        1
    }

    #[cfg(feature = "download")]
    {
        let cache = match paths::data_dir() {
            Ok(dir) => dir,
            Err(err) => {
                eprintln!("meditate: {err}");
                return 1;
            }
        };
        let fetcher = pack::HttpFetcher::new();
        let manifest = match pack::fetch_manifest(&fetcher, pack::DEFAULT_BASE_URL) {
            Ok(manifest) => manifest,
            Err(err) => {
                eprintln!("meditate: {err}");
                return 1;
            }
        };
        let assets = manifest.assets_for(kind);
        if assets.is_empty() {
            println!("No {} available right now.", kind.dir());
            return 0;
        }
        let mut failures = 0;
        for asset in assets {
            match pack::download(&fetcher, pack::DEFAULT_BASE_URL, &cache, kind, &asset.id) {
                Ok(path) => println!("Downloaded {} → {}", asset.id, path.display()),
                Err(err) => {
                    eprintln!("meditate: {} — {err}", asset.id);
                    failures += 1;
                }
            }
        }
        i32::from(failures > 0)
    }
}

fn cmd_integration(action: &IntegrationAction) -> i32 {
    let Some(base) = directories::BaseDirs::new() else {
        eprintln!("meditate: could not find your home directory");
        return 1;
    };
    let home = base.home_dir();
    let binary = std::env::current_exe()
        .ok()
        .and_then(|path| path.to_str().map(String::from))
        .unwrap_or_else(|| "meditate".to_string());

    let (verb, result) = match action {
        IntegrationAction::Install => ("Updated", integration::install(home, &binary)),
        IntegrationAction::Uninstall => ("Cleaned", integration::uninstall(home, &binary)),
    };
    match result {
        Ok(changed) if changed.is_empty() => {
            println!("No shell or tmux config found to update.");
            0
        }
        Ok(changed) => {
            for path in changed {
                println!("{verb} {}", path.display());
            }
            if matches!(action, IntegrationAction::Install) {
                println!("Restart your shell (or re-source it) to enable breathe nudges.");
            }
            0
        }
        Err(err) => {
            eprintln!("meditate: {err}");
            1
        }
    }
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
