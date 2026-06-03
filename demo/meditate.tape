# Demo recording for the README.
# Regenerate the GIF with:  vhs demo/meditate.tape
# (install VHS first: brew install vhs — https://github.com/charmbracelet/vhs)
#
# Run from the repo root. Uses the debug binary; build it once with:
#   cargo build --features audio,download

Output demo/meditate.gif

Set FontSize 22
Set Width 900
Set Height 540
Set Padding 20
Set Framerate 20

# Isolate config/streak so the recording is clean and deterministic.
Hide
Type "export XDG_DATA_HOME=/tmp/vhs-meditate XDG_CONFIG_HOME=/tmp/vhs-meditate/cfg"
Enter
Type "./target/debug/meditate equal"
Enter
Sleep 2s
Show

# Breathe — one full inhale/exhale of the 4/4 pattern, status bar visible.
Sleep 8s
# Switch pattern — the HUD updates to the new pattern name.
Type "n"
Sleep 5s
# Focus mode — the chrome falls away, just the orb.
Type "f"
Sleep 4s
Type "q"
Sleep 1s
