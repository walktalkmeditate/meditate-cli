# meditate

A terminal breathing companion — paced breathing, soundscapes, and voice guides,
right where you already work. Open it in the mid-day drag and breathe through the
next twenty minutes without reaching for your phone.

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

**Sound packs** (optional, opt-in):

```sh
meditate download soundscapes
meditate download voices
```

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
