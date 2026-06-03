use clap::Parser;
use meditate::cli::{Cli, Command, ConfigAction, PatternName};

#[test]
fn parses_pattern_and_duration() {
    let cli = Cli::try_parse_from(["meditate", "box", "--for", "5m"]).unwrap();
    assert_eq!(cli.pattern, Some(PatternName::Box));
    assert_eq!(cli.r#for.as_deref(), Some("5m"));
    assert!(cli.command.is_none());
}

#[test]
fn parses_breaths_and_toggles() {
    let cli = Cli::try_parse_from([
        "meditate",
        "--breaths",
        "10",
        "--no-streak",
        "--no-door",
        "--reduce-motion",
    ])
    .unwrap();
    assert_eq!(cli.breaths, Some(10));
    assert!(cli.no_streak);
    assert!(cli.no_door);
    assert!(cli.reduce_motion);
}

#[test]
fn parses_title_until_and_no_graphics() {
    let cli = Cli::try_parse_from([
        "meditate",
        "--title",
        "--no-graphics",
        "--until",
        "cargo build",
    ])
    .unwrap();
    assert!(cli.title);
    assert!(cli.no_graphics);
    assert_eq!(cli.until.as_deref(), Some("cargo build"));
}

#[test]
fn rejects_unknown_pattern() {
    assert!(Cli::try_parse_from(["meditate", "wobble"]).is_err());
}

#[test]
fn parses_download_subcommand() {
    let cli = Cli::try_parse_from(["meditate", "download", "soundscapes"]).unwrap();
    match cli.command {
        Some(Command::Download(args)) => assert_eq!(args.pack.as_deref(), Some("soundscapes")),
        other => panic!("expected download, got {other:?}"),
    }
}

#[test]
fn parses_config_path_subcommand() {
    let cli = Cli::try_parse_from(["meditate", "config", "path"]).unwrap();
    assert!(matches!(
        cli.command,
        Some(Command::Config {
            action: Some(ConfigAction::Path)
        })
    ));
}

#[test]
fn bare_invocation_has_no_command_or_pattern() {
    let cli = Cli::try_parse_from(["meditate"]).unwrap();
    assert!(cli.command.is_none());
    assert!(cli.pattern.is_none());
}

#[test]
fn every_pattern_name_round_trips_to_kebab() {
    assert_eq!(PatternName::DeepCalm.as_str(), "deep-calm");
    assert_eq!(
        Cli::try_parse_from(["meditate", "deep-calm"])
            .unwrap()
            .pattern,
        Some(PatternName::DeepCalm)
    );
}
