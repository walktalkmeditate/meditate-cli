# meditate

A terminal breathing companion — paced breathing, soundscapes, and voice guides,
right where you already work. Open it in the mid-day drag and breathe through the
next twenty minutes without reaching for your phone.

[![Release](https://img.shields.io/github/v/release/walktalkmeditate/meditate-cli?color=2f9e44&label=release)](https://github.com/walktalkmeditate/meditate-cli/releases)
[![CI](https://github.com/walktalkmeditate/meditate-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/walktalkmeditate/meditate-cli/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-2f9e44.svg)](LICENSE)

![meditate breathing in the terminal](demo/meditate.gif)

```
meditate
```

A moss-colored orb breathes in time with you; press a key to switch patterns or
sounds; press `q` when the meeting picks back up.

## Install

**Homebrew (macOS / Linux)**

```sh
brew install walktalkmeditate/tap/meditate
```

**One-line installers**

```sh
# macOS / Linux
curl -fsSL https://raw.githubusercontent.com/walktalkmeditate/meditate-cli/main/install.sh | sh
```

```powershell
# Windows
irm https://raw.githubusercontent.com/walktalkmeditate/meditate-cli/main/install.ps1 | iex
```

**From source** (Rust 1.82+)

```sh
cargo install --path . --features audio,download
```

The core breathing experience works with **zero downloads**. Sound packs are
optional and only fetched when you ask.

## Use

```sh
meditate                 # resume your last pattern, open-ended
meditate box             # start with a specific pattern
meditate --for 5m        # a timed session, ending with a soft bell
meditate --breaths 10    # end after ten breaths
meditate --reduce-motion # calmer, slower motion
```

While breathing: `n` next pattern · `s` soundscape · `v` voice · `b` bell ·
`m` mute · `+`/`-` volume · `space` pause · `f` focus · `q` quit (Ctrl-C also
quits gracefully).

**Patterns:** Calm (5/7) · Equal (4/4) · Relaxing (4-7-8) · Box (4-4-4-4) ·
Coherent (5/5) · Deep calm (3/6) · None (open focus).

**Sound packs** (optional — the breathing and a synthesized bell need no
downloads at all):

```sh
meditate download soundscapes   # ambient loops      — press s to cycle
meditate download voices        # meditation guides   — press v to cycle
meditate download bells         # start/end bells     — press b to cycle (synth stays the default)
```

Re-running a download only fetches what you don't already have. Voice packs pull
their meditation prompts only — walk guidance is never downloaded.

**Other commands:** `meditate config` · `meditate streak` ·
`meditate integration install` (shell/tmux breathe nudges).

## Customize

Edit `~/.config/meditate/config.toml` (created on demand) to set a default
pattern, pin the palette, rebind keys, or turn features off. Every key is
optional — zero-config still launches instantly. Run `meditate config path` to
find it.

## Privacy

No account, no telemetry, no background network. The only network call meditate
ever makes is a pack download you explicitly ask for.

## License

MIT. A gift to the terminal community — and a quiet door to the
[Pilgrim](https://pilgrimapp.org) app if you'd like to keep walking with it.

---

<sub>The demo above is generated with [VHS](https://github.com/charmbracelet/vhs):
`vhs demo/meditate.tape`.</sub>
