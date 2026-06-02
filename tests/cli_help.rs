use std::process::Command;

#[test]
fn version_reports_the_binary() {
    let output = Command::new(env!("CARGO_BIN_EXE_meditate"))
        .arg("--version")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("meditate"));
}

#[test]
fn help_lists_the_surface() {
    let output = Command::new(env!("CARGO_BIN_EXE_meditate"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("breathing companion"));
    assert!(text.contains("download"));
    assert!(text.contains("config"));
    assert!(text.contains("integration"));
    assert!(text.contains("streak"));
}
