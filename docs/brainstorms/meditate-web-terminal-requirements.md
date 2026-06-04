# meditate web terminal — requirements

Created: 2026-06-03

## Problem / Opportunity

meditate (the CLI) has no zero-friction way to try it — you must install first.
A "terminal in the browser" at `cli.pilgrimapp.org` removes that wall: click a
link, breathe, the sound is already loaded, no account. One static site serves
three jobs at once — a real local-first web product, a self-updating interactive
demo, and a subtle funnel to both the CLI and the Pilgrim app — and the whole
identity of meditate (a free, no-account, no-telemetry gift that's a soft door to
Pilgrim) extends naturally to the web.

## Primary outcome

A visitor lands and within ~3 seconds is breathing with a beautiful orb in an
authentic terminal — no install, no account — and leaves having either installed
the CLI, bookmarked the web app, or grown curious about Pilgrim, without ever
being "marketed" to.

## Actors

- **A1 — First-time visitor** (a dev who clicked a shared link, desktop or mobile): wants to instantly feel what this is.
- **A2 — Returning web user**: breathes in a browser tab; streak + last setup remembered locally.
- **A3 — CLI-curious dev**: discovers via terminal-native hints that meditate runs for real in their terminal, and copies the install command.
- **A4 — Pilgrim-curious visitor**: notices, faintly, that the same folks make Pilgrim.

## Key flows

- **F1 — Land & breathe**: page loads → a ~1s "boot" → the orb fades in breathing (remembered pattern, else `calm`). Watch, or type.
- **F2 — Drive it like the CLI**: a live prompt accepts the real commands — `meditate box --for 5m`, `sound forest`, `voice`, `bell`, `theme dusk`, `streak`, `graphics`, `help`, `man meditate`, `clear`, `share`, `install`, `whoami`. `↑` history, tab-completion.
- **F3 — Deep-link**: `cli.pilgrimapp.org/box?for=5m&sound=forest` opens straight into that session; `share` copies the current link.
- **F4 — Discover the CLI**: a login-style MOTD banner + `install` / `which meditate` / `brew` commands + a post-session comment surface the brew line, copy-on-click.
- **F5 — Local persistence**: streak + last pattern/volume/sound in `localStorage` (no account); `streak` renders a block-char calendar heatmap in the terminal.

## Requirements

- **R1.** Authentic terminal UI (a real terminal emulator), keyboard-first, with a working prompt that mirrors the CLI's command surface — so using the site teaches the CLI.
- **R2.** The breathing orb is driven by the actual Rust breath/orb core compiled to WASM — single source of truth for the curves and math (no JS re-implementation of the breathing logic).
- **R3.** Two orb renders: **half-block** (default, the exact terminal render) and a **smooth orb** toggled by `graphics`. The smooth orb visually matches the **Pilgrim iOS meditation orb** — layered moss radial gradients (soft outer halo + bright inner core), `easeInOut` breath scale, moss ripple rings, ambient drifting particles. Reference: `../pilgrim-ios/Pilgrim/Scenes/ActiveWalk/MeditationView.swift`.
- **R4.** All audio (soundscapes/voices/bells) loaded live from the CDN via Web Audio — no download step — plus the synth bell for zero-latency.
- **R5.** Local-first persistence (streak, last config) in `localStorage`. No account, no telemetry, no backend.
- **R6.** Subtle, terminal-native CLI-install discovery — MOTD banner, `install`/`which`/`brew` commands, copy-on-click of any command text, a faint post-session hint. Never a marketing banner or popup.
- **R7.** The faintest Pilgrim hint, surfaced only in `whoami`/`credits`/`about`, never on the main surface.
- **R8.** Deep-linkable sessions via URL params + a `share` command that copies the current link.
- **R9.** Craft polish: a boot sequence, seasonal/time-of-day palette (from the core), the **browser tab title breathes** (the `--title` block animation, native to a tab), and `prefers-reduced-motion` respected.
- **R10.** Mobile/touch usable (shared links get clicked on phones): the orb and essential controls work without a keyboard.
- **R11.** `man meditate` / `help` render the CLI's real documentation — single source for both web and CLI.
- **R12.** An embeddable "breathe break" web component other sites can drop in.

## Scope boundaries

**Deferred for later (aligned, not v1):**
- The embeddable web component (R12) — ship the destination first.
- Full pack-browsing UI beyond the core set.

**Outside this product's identity (won't build):**
- Accounts, login, cloud sync, cross-device — breaks no-account / local-first.
- Co-breathing / sync rooms — needs a server; breaks the no-backend identity.
- Any telemetry/analytics that phones home.
- Marketing chrome — popups, modal banners, email capture.

## Dependencies / Assumptions

- **CDN CORS**: `cdn.pilgrimapp.org` must serve headers allowing the `cli.pilgrimapp.org` origin (ops; user-controlled Cloudflare/R2).
- **DNS + hosting**: GitHub Pages with a `CNAME` for `cli.pilgrimapp.org`.
- **WASM**: the Rust pure core (breath, orb, palette, renderers) compiles to `wasm32` — confirmed, those modules have no I/O deps.
- **Audio decode**: the browser's Web Audio decodes the `.aac` packs natively.
- Assumption: one first-load of audio (or lazy per-selection) is acceptable — confirm in planning.

## Success criteria

- A visitor breathes within ~3s of landing — no install, no account.
- The same commands work on web and CLI; using the site is hands-on CLI practice.
- The install nudge is discoverable but never reads as an ad.
- Beautiful enough to earn an unsolicited "whoa" (HN / dev-Twitter worthy).
- Zero backend, zero account, zero telemetry — the meditate identity stays intact.

## Open questions (for planning)

- Smooth-orb background: keep the terminal-dark frame, or adopt the iOS parchment background for a fuller match?
- Audio: preload everything vs on-demand per selection; exact CORS config.
- Repo layout + deploy: a `web/` directory in `meditate-cli` (GitHub Pages via Actions) vs a separate repo.
- Mobile interaction model without a keyboard: which controls surface, and how.
- WASM toolchain (wasm-bindgen / trunk / a Vite + xterm.js shell) — planning's call.
