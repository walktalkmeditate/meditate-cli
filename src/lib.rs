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
/// Precedence: a pattern named on the command line wins; then a pinned
/// `default_pattern` from config; then — when `resume_last_pattern` is enabled
/// (the default) — the last-used pattern remembered in machine state. `None`
/// means no source applied, and the caller falls back to the built-in default.
pub fn resolve_start_pattern(
    cli_pattern: Option<&str>,
    config: &Config,
    state: &State,
) -> Option<String> {
    if let Some(pattern) = cli_pattern {
        return Some(pattern.to_string());
    }
    // A config default is a pinned preference and wins over session memory.
    if let Some(pattern) = &config.default_pattern {
        return Some(pattern.clone());
    }
    if config.resume_last_pattern() {
        if let Some(pattern) = &state.last_pattern {
            return Some(pattern.clone());
        }
    }
    None
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
        match kind {
            pack::AssetKind::Voice => download_voice_packs(&fetcher, &cache),
            _ => download_audio_packs(&fetcher, &cache, kind),
        }
    }
}

#[cfg(feature = "download")]
fn download_audio_packs(
    fetcher: &dyn pack::Fetcher,
    cache: &std::path::Path,
    kind: pack::AssetKind,
) -> i32 {
    let manifest = match pack::fetch_audio_manifest(fetcher) {
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
        match pack::download_audio_asset(fetcher, cache, kind, asset) {
            Ok(pack::DownloadOutcome::Downloaded(path)) => {
                println!("Downloaded {} → {}", asset.id, path.display())
            }
            Ok(pack::DownloadOutcome::AlreadyCached(_)) => {
                println!("Already have {}", asset.id)
            }
            Err(err) => {
                eprintln!("meditate: {} — {err}", asset.id);
                failures += 1;
            }
        }
    }
    i32::from(failures > 0)
}

#[cfg(feature = "download")]
fn download_voice_packs(fetcher: &dyn pack::Fetcher, cache: &std::path::Path) -> i32 {
    let manifest = match pack::fetch_voice_manifest(fetcher) {
        Ok(manifest) => manifest,
        Err(err) => {
            eprintln!("meditate: {err}");
            return 1;
        }
    };
    let packs: Vec<_> = manifest
        .packs
        .iter()
        .filter(|pack| !pack.meditation_prompts.is_empty())
        .collect();
    if packs.is_empty() {
        println!("No voices available right now.");
        return 0;
    }
    let mut failures = 0;
    for voice_pack in packs {
        match pack::download_voice_pack_from(fetcher, cache, voice_pack) {
            Ok(pack::DownloadOutcome::Downloaded(dir)) => {
                println!("Downloaded {} → {}", voice_pack.id, dir.display())
            }
            Ok(pack::DownloadOutcome::AlreadyCached(_)) => {
                println!("Already have {}", voice_pack.id)
            }
            Err(err) => {
                eprintln!("meditate: {} — {err}", voice_pack.id);
                failures += 1;
            }
        }
    }
    i32::from(failures > 0)
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
        Some(ConfigAction::Init { force }) => config_init(&path, *force),
        _ => config_show(&path),
    }
}

/// Print the config file, or — when there is none — the default template so the
/// user can see every option, with a hint to write it.
fn config_show(path: &std::path::Path) -> i32 {
    match std::fs::read_to_string(path) {
        Ok(text) => {
            print!("{text}");
            0
        }
        Err(_) => {
            print!("{}", config::default_template());
            println!("# No config file yet — the template above lists every option.");
            println!("# Write it with:  meditate config init");
            0
        }
    }
}

/// Write the commented template to the config path, refusing to clobber an
/// existing file unless `--force` was given.
fn config_init(path: &std::path::Path, force: bool) -> i32 {
    if path.exists() && !force {
        eprintln!(
            "meditate: a config already exists at {} (pass --force to overwrite)",
            path.display()
        );
        return 1;
    }
    if let Some(parent) = path.parent() {
        if let Err(err) = std::fs::create_dir_all(parent) {
            eprintln!("meditate: could not create config directory: {err}");
            return 1;
        }
    }
    match std::fs::write(path, config::default_template()) {
        Ok(()) => {
            println!("Wrote {}", path.display());
            println!("Open it to set your defaults — every option is commented with its default.");
            0
        }
        Err(err) => {
            eprintln!("meditate: could not write config: {err}");
            1
        }
    }
}
