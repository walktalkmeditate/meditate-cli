use meditate::integration::{
    apply_install, apply_uninstall, shell_escape, targets, with_block, without_block,
};
use std::path::Path;

fn snippet_for(label: &str) -> String {
    targets(Path::new("/home/dev"), "/usr/local/bin/meditate")
        .into_iter()
        .find(|t| t.label == label)
        .unwrap()
        .snippet
}

#[test]
fn shell_escape_wraps_and_escapes_quotes() {
    assert_eq!(shell_escape("/a b/meditate"), "'/a b/meditate'");
    assert_eq!(shell_escape("it's"), "'it'\\''s'");
}

#[test]
fn block_round_trips_and_is_idempotent() {
    let original = "export PATH=/x";

    let installed = with_block(original, "SNIPPET_ONE");
    assert!(installed.contains("SNIPPET_ONE"));
    assert!(installed.contains(">>> meditate integration >>>"));
    assert_eq!(without_block(&installed), original);

    let reinstalled = with_block(&installed, "SNIPPET_TWO");
    assert!(reinstalled.contains("SNIPPET_TWO"));
    assert!(!reinstalled.contains("SNIPPET_ONE"));
    assert_eq!(without_block(&reinstalled), original);
}

#[test]
fn with_block_handles_empty_input() {
    let installed = with_block("", "SNIPPET");
    assert!(installed.starts_with("# >>> meditate integration >>>"));
    assert!(without_block(&installed).is_empty());
}

#[test]
fn shell_snippets_are_escaped_guarded_and_rate_limited() {
    for label in ["zsh", "bash"] {
        let snippet = snippet_for(label);
        assert!(snippet.contains("'/usr/local/bin/meditate'"));
        assert!(snippet.contains("__meditate_threshold=120"));
        assert!(snippet.contains("__meditate_cooldown=900"));
        assert!(snippet.contains("MEDITATE_NUDGE_OFF"));
        assert!(snippet.contains("pgrep -x meditate"));
    }
}

#[test]
fn tmux_snippet_binds_a_popup() {
    let snippet = snippet_for("tmux");
    assert!(snippet.contains("display-popup -E '/usr/local/bin/meditate'"));
}

#[test]
fn install_then_uninstall_restores_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let rc = dir.path().join(".zshrc");
    std::fs::write(&rc, "export EDITOR=vim\n").unwrap();

    apply_install(&rc, "SNIPPET").unwrap();
    let after_install = std::fs::read_to_string(&rc).unwrap();
    assert!(after_install.contains("SNIPPET"));
    assert!(after_install.contains("export EDITOR=vim"));

    apply_uninstall(&rc).unwrap();
    assert_eq!(
        std::fs::read_to_string(&rc).unwrap().trim_end(),
        "export EDITOR=vim"
    );
}
