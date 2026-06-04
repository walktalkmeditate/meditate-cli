---
status: active
type: feat
origin: docs/brainstorms/meditate-web-terminal-requirements.md
created: 2026-06-03
reviewed: 2026-06-03
---

# feat: meditate web terminal (cli.pilgrimapp.org)

A no-account, local-first "terminal in the browser" that runs the **actual Rust
breath/orb core compiled to WASM** inside xterm.js, where users type the real
`meditate` commands. One static site = a craft showpiece + a genuine web app +
a self-updating CLI demo + a subtle funnel to the CLI and Pilgrim.

Origin requirements: `docs/brainstorms/meditate-web-terminal-requirements.md`.
Revised after a multi-persona document review (foundation, scope, and design
findings folded in).

---

## Problem Frame & Approach

meditate (the CLI) has no zero-friction way to try it — you must install. The web
terminal removes that wall (click → breathe; the *visual* orb starts immediately,
sound on the first interaction) while staying true to meditate's identity (free,
no-account, no-telemetry, a soft door to Pilgrim).

**The through-line — and its honest boundary:** one source of truth — the Rust
breath engine — feeds the renderers off a single `tick(dt)`. That sharing is real
for the **breath curves, the half-block orb, the OSC title bytes, and the
help/`man` text** (all compiled from `meditate-core`). It is **not** shared for the
REPL, audio scheduling, streak math, or deep-link parsing — those are new
TypeScript with no Rust source. Those four surfaces are kept in parity with the
CLI **by characterization tests**, not by the compiler (the compiler can't catch a
drifted pattern alias, duck ramp, or midnight streak boundary).

**Why a workspace is required, not optional:** the current `meditate` crate depends
on crossterm/cpal/ureq, none of which target `wasm32`. The pure render modules must
move into a dependency-free `meditate-core` crate so a tiny `meditate-wasm` façade
can compile them to WASM. The CLI package **stays at the repo root** so the
existing release pipeline, install scripts, and Homebrew tap are untouched.

---

## Key Technical Decisions

- **Toolchain:** `wasm-bindgen` (lib + CLI, versions pinned identical; CLI MSRV 1.82 matches) → `--target web` → `wasm-opt -Oz`, consumed by **Vite + TypeScript** via `vite-plugin-wasm` + `vite-plugin-top-level-await`. Not wasm-pack (archived Jul 2025) or trunk.
- **Repo shape:** Cargo workspace; root stays the `meditate` CLI; add `crates/meditate-core` (pure) and `crates/meditate-wasm` (cdylib façade); the site lives in `web/`.
- **Panic profile:** Cargo ignores `[profile.*]` in non-root workspace members. So the wasm crate's `panic="abort"` is set via the build invocation (`RUSTFLAGS="-C panic=abort"` on the `wasm32-unknown-unknown` build), **not** a member-crate profile. The root `[profile.release]` keeps `unwind` for the CLI's `TerminalGuard`.
- **WASM surface (one name throughout):** an opaque `#[wasm_bindgen]` session handle holding a `Breath` + palette. Methods: `new(pattern, month, hour)`, `set_pattern(name)`, `pause_toggle()`, `tick_frame(elapsed_ms, cols, rows) -> String` (ANSI, incl. the OSC-0 title bytes), and **breath-state accessors** the canvas + audio read each frame: `fullness() -> f32`, `glow() -> f32`, `phase_label() -> String`, `breath_count() -> u32`, `palette() -> [u8; 9]` (core/edge/ripple RGB). **There is no RGBA export** — the smooth orb is drawn in Canvas-2D from these accessors (see U6), which keeps `render/graphics.rs` fully native and avoids a per-frame pixel-buffer boundary.
- **Terminal:** `@xterm/xterm` 6.x (scoped packages) + `@xterm/addon-webgl` (essential for truecolor at 30fps) + `@xterm/addon-fit`. Anti-flicker = Synchronized Output (`\x1b[?2026h … \x1b[?2026l`) + cursor-home overwrite (never `\x1b[2J`) + one `term.write()` per frame, via `requestAnimationFrame` throttled to ~24–30fps and **dt-scaled** (clamp dt spikes on tab refocus). `onTitleChange → document.title` animates the real browser tab.
- **Smooth orb (decided):** Canvas-2D `createRadialGradient` for an **exact iOS match**, layered over the terminal (`position:absolute`, `pointer-events:none`, `devicePixelRatio`-aware). It reads the WASM accessors (one clock → no drift). Background stays **terminal-dark** (the orb overlays the dark terminal — preserves the terminal aesthetic; iOS's parchment background is *not* adopted). `graphics` toggles block↔smooth, mirroring the CLI's `--no-graphics`.
- **Palette (R9):** the core has no clock, so JS injects the current `month`/`hour` into `Session::new`; the core's `season_for_month`/`time_for_hour`/`resolve_with_pin` (now in `meditate-core`) pick the seasonal/time-of-day palette, exposed via `palette()` for the canvas.
- **Audio:** single `AudioContext`; `fetch` → `decodeAudioData` of the CDN `.aac` packs; `loop=true` + per-source `GainNode`; equal-power crossfade and scheduled-ramp ducking. Gesture-gated `resume()`; lazy-load + background-prefetch.
- **Persistence:** one `localStorage` JSON blob (`schemaVersion`, prefs, date-keyed completions) with a **single absent/corrupt initializer** (no migration ladder until a v2 schema exists); derive streak + a 30-day rate; `export`/`import`.
- **Deep-links:** **hash** fragment `#p=box&d=5m&snd=rain` (never sent to the server → never 404s on Pages) — human-readable. This **supersedes** origin F3's `?for=…` query example and R8's "URL params" wording (equivalent for the user, server-safe).
- **Deploy:** GitHub Actions (`upload-pages-artifact` + `deploy-pages`), custom domain in repo **Settings** (Actions deploys ignore a committed `CNAME`), `base:'/'`, Enforce HTTPS; DNS CNAME `cli` → `<org>.github.io`.
- **CORS:** R2 bucket returns `Access-Control-Allow-Origin: https://cli.pilgrimapp.org` for `GET`/`HEAD`; **purge Cloudflare cache** after.

---

## Output Structure

```
meditate-cli/                       # repo root stays the CLI package + workspace root
├── Cargo.toml                      # [workspace] members + the existing meditate [package]
├── src/                            # CLI unchanged in place; imports meditate_core::*; re-exports core types
│   ├── session.rs, term.rs, audio/, pack/, cli.rs, config.rs, paths.rs, wait.rs, streak.rs, keymap.rs, state.rs
│   ├── render/graphics.rs          # STAYS native (kitty/iTerm2 escapes; not used by the web)
│   └── (breath.rs, render/{mod,orb,cell_gradient,mono}.rs, palette.rs, title.rs REMOVED → meditate-core)
├── crates/
│   ├── meditate-core/              # PURE, dep-free, wasm-safe
│   │   ├── Cargo.toml
│   │   └── src/{lib.rs, breath.rs, render/{mod,orb,cell_gradient,mono}.rs, palette.rs, title.rs, caps.rs}
│   └── meditate-wasm/              # crate-type = ["cdylib"]; wasm-bindgen façade
│       ├── Cargo.toml
│       └── src/lib.rs
├── web/
│   ├── index.html, package.json, package-lock.json, vite.config.ts, tsconfig.json
│   ├── src/{main.ts, terminal.ts, loop.ts, repl.ts, audio.ts, orb-canvas.ts, store.ts, deeplink.ts, motd.ts, mobile.ts, boot.ts}
│   ├── src/commands/               # grouped, NOT one file per command (split a command out only past ~40 lines)
│   ├── src/wasm/                   # generated wasm-bindgen output (build artifact, gitignored)
│   └── public/                     # fonts (preloaded), favicon
└── .github/workflows/{ci.yml, pages.yml}   # ci.yml gains a wasm32 build gate; pages.yml deploys
```

The per-unit `**Files:**` sections remain authoritative; the implementer may adjust layout.

---

## Implementation Units

### Phase A — Foundation (a **deployable checkpoint**: the orb breathing in a browser at the real domain, to de-risk U1 before B/C is stacked on it)

### U1. Extract the pure core into `meditate-core`, decoupling the real cross-crate edges

**Goal:** Move the I/O-free render modules into a dep-free crate — handling the two non-mechanical edges the review found — with the CLI behavior unchanged.
**Requirements:** R2; enables R1/R3/R9.
**Dependencies:** none.
**Files:**
- Root `Cargo.toml`: add `[workspace] members = ["crates/*"]` (root stays the `meditate` `[package]`; root `[profile.release]` keeps `unwind`).
- Create `crates/meditate-core/{Cargo.toml, src/lib.rs}` (deps: `std` only; no clap/serde/crossterm).
- Move `src/breath.rs`, `src/render/{mod,orb,cell_gradient,mono}.rs`, `src/palette.rs`, `src/title.rs` → `crates/meditate-core/src/`.
- **Decouple edge 1 (term):** create `crates/meditate-core/src/caps.rs` holding the pure data types `ColorDepth`, `GraphicsProtocol`, `Capabilities`. `meditate-core`'s render uses `caps::Capabilities`. The CLI's `src/term.rs` keeps `Env`/`SystemEnv`/`MapEnv` + the `detect_*` functions (which use `std::env`) and **re-exports** the core types (`pub use meditate_core::caps::{Capabilities, ColorDepth, GraphicsProtocol};`).
- **Decouple edge 2 (palette pin):** add a clap-free `palette::Pin` enum (spring/summer/autumn/winter/dawn/day/dusk/night) to `meditate-core`; `resolve_with_pin` takes `Option<Pin>`. The CLI's `cli::PalettePin` (clap `ValueEnum`) gains `From<PalettePin> for meditate_core::palette::Pin` and maps at the call site.
- **`render/graphics.rs` stays in the CLI** (native kitty/iTerm2 escapes); `src/state.rs` **stays in the CLI** (native session memory; not moved).
- Modify CLI modules referencing the moved paths (`src/session.rs`, `src/lib.rs`, `src/keymap.rs`, …) to use `meditate_core::*`. The CLI's `lib.rs` **re-exports** the moved modules under their old public paths (`pub use meditate_core::{breath, render, palette, title};`) so existing integration tests (`tests/palette.rs`, `tests/render_degradation.rs`) compile unchanged.
- Add a **wasm32 build gate** to `.github/workflows/ci.yml`: `rustup target add wasm32-unknown-unknown` + `cargo build -p meditate-core --target wasm32-unknown-unknown` (extended to `meditate-wasm` in U2), so a stray `std::env`/`Instant` in core fails on the PR, not at deploy.
**Approach:** Two real edges (`palette.rs → crate::cli::PalettePin`, `render → crate::term::Capabilities`) make this **not** a pure file move — the decoupling above is the substance of U1. Everything else (breath, orb, mono, title) is genuinely clean. The CLI re-exports keep its public surface and tests stable.
**Execution note:** Characterization-safe refactor — the full CLI suite (`cargo test` + `--features audio,download`) + clippy + fmt must stay green; treat any CLI behavior change as a mistake.
**Test scenarios:**
- The moved Rust unit tests (breath, orb, renderer encode, palette) pass inside `meditate-core`, including for `wasm32-unknown-unknown` (`cargo build -p meditate-core --target wasm32-unknown-unknown` succeeds — no `std::env`/`Instant`).
- The full CLI suite (default + `--features audio,download`) and `tests/palette.rs` + `tests/render_degradation.rs` pass **unchanged** via the re-exports.
- `cargo build` from root still emits the `meditate` binary; the release workflow path is unaffected.
**Verification:** Workspace builds (native + wasm32 for core); CLI binary + all existing tests behave identically; `meditate-core` has no clap/crossterm/cpal/ureq dependency; CI gates wasm32.

### U2. The `meditate-wasm` façade + WASM build pipeline

**Goal:** A tiny crate exposing the session handle to JS, plus a reproducible build.
**Requirements:** R2, R3 (the accessors that drive both orbs).
**Dependencies:** U1.
**Files:**
- Create `crates/meditate-wasm/{Cargo.toml, src/lib.rs}` (`crate-type=["cdylib"]`; deps: `meditate-core`, `wasm-bindgen` pinned).
- The build script `web/scripts/build-wasm.sh`: `RUSTFLAGS="-C panic=abort" cargo build --release --target wasm32-unknown-unknown -p meditate-wasm` → `wasm-bindgen --target web --out-dir web/src/wasm …` → `wasm-opt -Oz`. The wasm-bindgen **lib version and the `cargo binstall`'d CLI version read from one pinned source**; CI fails on skew.
- Extend the `ci.yml` wasm gate (U1) to also `cargo build -p meditate-wasm --target wasm32-unknown-unknown`.
**Approach:** State lives in WASM (the opaque handle); JS crosses the boundary once per frame for `tick_frame` (returns one ANSI string) and reads cheap scalar/`palette()` accessors. `tick_frame` reuses `cell_gradient`/`mono` encode + the `title` OSC. **No RGBA export** (the canvas orb is Canvas-2D from accessors), so `graphics.rs` and its `rgba()` helper stay entirely native — the U1↔U2 contradiction the review flagged does not arise.
**Patterns to follow:** the native `session.rs` draw path (scene → encode) for `tick_frame`; prior art `aschey/ratatui-xterm-js` for the wiring.
**Test scenarios:**
- A native Rust test in `meditate-wasm` drives `Session::new(...).tick_frame(...)` and asserts non-empty ANSI containing truecolor + `▀` and an OSC-0 title sequence; `fullness()` rises on inhale and falls on exhale; `palette()` returns 9 bytes.
- `web/scripts/build-wasm.sh` produces a `.wasm` + typed `.d.ts`; lib/CLI version parity is asserted (the script fails on mismatch).
**Verification:** `web/src/wasm/` holds the built module + `.d.ts`; the façade compiles native (tests) and `wasm32`; the `.wasm` is low-tens-of-KB after `-Oz`.

### U3. Web app shell: the breathing orb in xterm.js, with designed loading + first-paint

**Goal:** A Vite + TS site that loads the WASM and renders the breathing half-block orb smoothly, with no blank/flash and a clear "this is interactive" cue. **This is the Phase-A deployable checkpoint.**
**Requirements:** R1, R2, R9 (tab title + palette + reduced-motion).
**Dependencies:** U2.
**Files:** `web/{package.json, vite.config.ts, tsconfig.json, index.html}`; `web/src/{main.ts, terminal.ts, loop.ts}`.
**Approach:**
- **Loading states (designed, not default):** `index.html` ships a static pre-WASM placeholder (a centered dim ASCII orb glyph + a faint "loading…") so there's never a blank white page during WASM init. The block font is **preloaded** (`<link rel="preload">`, `font-display:block`) to avoid an xterm reflow/flash. The terminal/orb appear only once WASM + font are ready.
- **First-paint affordance (designed):** after the boot (U8), the orb breathes silently and a **blinking prompt with a ghosted hint** (`type a command, or press any key` on desktop; the chip row on touch) makes "this responds to me" obvious. The first keypress/tap is the audio-unlock gesture.
- **Render:** rAF → `session.tick_frame(elapsed_ms, cols, rows)` → one `term.write()` wrapped in `?2026h…?2026l`, cursor home, cursor hidden during animation. WebGL addon; `onResize` → cols/rows (debounced); `onTitleChange → document.title` (the tab breathes). `prefers-reduced-motion` slows the cadence.
- **Palette (R9):** `main.ts` passes the current `month`/`hour` into `Session::new` so the seasonal/time-of-day palette is live.
**Patterns to follow:** the research anti-flicker rules; `aschey/ratatui-xterm-js`.
**Test scenarios:**
- Vitest: the frame-writer wraps in BSU/ESU, homes the cursor, never emits `\x1b[2J`.
- Vitest: the loop throttles to ~≤30fps, scales motion by dt, and clamps a large refocus dt.
- Vitest: `Session::new` receives a plausible month/hour and the palette differs across two injected times.
- (Manual/visual) no blank/flash on load; the orb breathes smoothly; the tab title animates; a first-time visitor sees a clear "type/tap" affordance.
**Verification:** Deployed to Pages at the real domain (the Phase-A checkpoint): the orb breathes with a designed load + first-paint and an animating tab title.

---

### Phase B — The terminal product

### U4. The REPL / command surface (mirrors the CLI) + complete mobile flow + MOTD

**Goal:** A working prompt (history, command-name completion), a MOTD with real copy, and a **complete** touch flow.
**Requirements:** R1, R6 (MOTD), R10 (mobile), R11 (help/man).
**Dependencies:** U3.
**Files:** `web/src/{repl.ts, mobile.ts, motd.ts}`; `web/src/commands/` (grouped dispatch — `meditate`, `sound`, `voice`, `bell`, `theme`, `pause`, `graphics`, `clear`, `help`, `man`, `which`, plus the soft-discovery group `install`, `whoami`, `share`, `streak`; one dispatch file + a soft-discovery file, splitting a command out only past ~40 lines — **not** 9 thin files).
**Approach:**
- Input is **decoupled** from the render tick: `onData` mutates shell state immediately; the rAF loop only reads breath state. Commands mirror `src/cli.rs` + `src/keymap.rs`. Parity is enforced by tests (this is a re-implementation, not shared code).
- **Mobile (R10) — the full flow, not 4 chips:** a chip row covering every essential action to complete a session without a keyboard — `breathe`, `pattern ▸` (cycles), `sound ▸`, `voice`, `bell`, `pause`, `theme`, `share`, `end`, plus `install`. Tapping a chip **echoes the equivalent command** into the terminal (the user sees what happened) **and** satisfies the audio gesture. Chips are real focusable buttons (`tabindex`, `:focus-visible`, `inputmode`). Layout: the orb sits above a fixed bottom chip row (orb region shrinks on small viewports).
- **MOTD (real copy, not "a banner"):** a short login-style banner — line 1 an ASCII `meditate` wordmark; line 2 `<version> · local session · no account, nothing leaves your browser`; line 3 `type 'help' to begin · 'install' to run it in your real terminal`. Skippable; no color spam.
- `man`/`help` render the CLI's real help text (single source via the bundled help string).
**Patterns to follow:** Sat Naing terminal-portfolio (history/completion); terminal.shop (TUI funnel restraint); `src/cli.rs` command/flag names.
**Test scenarios:**
- Vitest: line editor handles enter/backspace, `↑`/`↓` history, `Tab` unique-prefix completion + ambiguous-list.
- Vitest: the parser maps `meditate box --for 5m` → `{pattern:"box", for:"5m"}` and rejects unknown patterns with a helpful message; **parity test** — the parser accepts exactly the patterns `src/cli.rs` accepts.
- Vitest: on a simulated touch env, every essential action is reachable via a focusable chip; tapping a chip echoes its command text.
**Verification:** Typing the real commands drives the session; a full session is completable on a phone via chips; the MOTD reads like a login banner.

### U5. Web Audio engine (live CDN packs) with designed failure states

**Goal:** Instant, gapless, crossfading, duckable audio from the CDN — with graceful, on-brand behavior when it can't play.
**Requirements:** R4.
**Dependencies:** U4.
**Files:** `web/src/audio.ts`.
**Approach:**
- One `AudioContext`; manifest loader building URLs exactly as `src/pack/mod.rs` (`{base}/{type}/{id}.aac`, `{voiceBase}/{packId}/{promptId}.aac`); `fetch`→`decodeAudioData` with an `AudioBuffer` cache; per-layer `GainNode`; equal-power crossfade; scheduled-ramp ducking (`cancelScheduledValues` → `setValueAtTime` → `linearRampToValueAtTime`); a synth-oscillator bell for zero latency. Meditation-only voice (ignore walk prompts), matching the CLI.
- **Gesture-gate:** on the first `onData`/chip-tap, `if (ctx.state!=='running') await ctx.resume()` + a 1-sample silent-buffer warm (iOS). Lazy-load the selected pack; background-prefetch the rest.
- **Failure states (designed, terminal-native):** CORS/offline/decode failure → a single dim line `audio unavailable — breathing continues` and **fall back to the synth bell**; the orb is never blocked. iOS hardware-mute → a one-time hint line `(system sound is muted)`. No raw error objects ever reach the terminal.
**Patterns to follow:** MDN Web Audio best-practices; the CDN contract in `src/pack/mod.rs`; the CLI mixer/duck in `src/audio/mod.rs` (parity target).
**Test scenarios:**
- Vitest (AudioContext mocked): manifest URLs match the CLI exactly; voice loading ignores walk prompts.
- Vitest: equal-power crossfade gains preserve ~constant power; the duck schedule cancels prior ramps before re-anchoring (**parity** with the CLI duck behavior).
- Vitest: a simulated fetch rejection routes to the "audio unavailable" line + synth-bell fallback (no thrown error reaches the REPL).
- (Manual, real iOS) audio starts only after a gesture; gapless loop; voice ducks; mute-switch hint shows.
**Verification:** `sound forest` loops after the first interaction; `voice` ducks it; nothing autoplays; failures degrade gracefully to silence-plus-bell with a calm hint.

### U6. The smooth orb canvas overlay (`graphics` toggle), exact iOS match

**Goal:** A smooth orb matching the Pilgrim iOS orb, drawn in Canvas-2D from the WASM breath accessors, toggled by `graphics`.
**Requirements:** R3.
**Dependencies:** U3 (the tick + accessors).
**Files:** `web/src/orb-canvas.ts`; modify `web/src/commands/` (add `graphics`) and `web/src/loop.ts` (route the orb region).
**Approach:**
- A `<canvas>` absolutely positioned over `term.element` (`pointer-events:none`, higher z-index, `devicePixelRatio`-scaled, resynced on `onResize`/scroll). Each frame it draws a `createRadialGradient` orb whose radius/opacity/glow come from the WASM accessors (`fullness`, `glow`) and whose colors come from `palette()` — **one clock, no drift**.
- **Exact iOS match (anti-slop):** before writing `orb-canvas.ts`, **extract the real constants from `../pilgrim-ios/Pilgrim/Scenes/ActiveWalk/MeditationView.swift`** — the gradient stop opacities + radii (outer halo `moss 0.5→0.15→0` at ~320pt, inner core `moss 0.7+glow→0.3` at ~160pt), the `easeInOut` curve, the ripple-ring stroke timing, and the particle count/size/opacity — and encode those values (do not approximate). Background stays terminal-dark.
- **Toggle UX:** `graphics` flips a flag; the swap is a short cross-fade (≤200ms), not a hard cut. In block mode the canvas is hidden and the orb region renders in xterm; in smooth mode the orb cells render as spaces and the canvas shows. `graphics` prints a one-line confirmation (`graphics: smooth` / `graphics: blocks`).
**Patterns to follow:** the iOS `MeditationView.swift` constants; the research overlay pattern (`pointer-events:none`, dpr, resync).
**Test scenarios:**
- Vitest: the breath→gradient mapping (fullness → inner radius/opacity) is monotonic and matches the orb-scale curve; the toggle flips one flag and the cross-fade is bounded.
- (Manual/visual) the smooth orb is recognizably the Pilgrim orb (constants extracted, not eyeballed), phase-locked with the breath, crisp on retina, never eats keystrokes, realigns on resize.
**Verification:** `graphics` cross-fades to an iOS-accurate smooth orb and back; input still works; no drift; the gradient constants trace to `MeditationView.swift`.

### U7. Local-first persistence, streak, and deep-links — with designed landing

**Goal:** Remember prefs + a streak locally (no account), render a familiar streak heatmap, and make sessions shareable — with a designed deep-link landing.
**Requirements:** R5, R8, R9 (streak craft).
**Dependencies:** U4.
**Files:** `web/src/{store.ts, deeplink.ts}`; modify `web/src/commands/streak.ts`.
**Approach:**
- `store.ts`: one versioned JSON blob (`{schemaVersion, prefs:{pattern,volume,sound,voice,bell,theme}, completions:{"YYYY-MM-DD":true}}`) with a **single absent/corrupt initializer** (defer a migration ladder to a real v2); `export`/`import`; streak + 30-day-rate derivation from the date-keyed map (**local** date; **parity** with `src/streak.rs`).
- **Streak heatmap (spec'd):** a GitHub-contribution-style grid — weeks as columns, ~26 weeks back, the ramp ` ·░▓█` for none→full, current-day highlighted, the streak count + 30-day rate printed above the grid.
- `deeplink.ts`: parse `location.hash` (`#p=box&d=5m&snd=rain`) on load; `share` builds + clipboard-copies the link; `replaceState` for refinements, `pushState` on entering a session.
- **Deep-link landing (designed):** a shared link lands **pre-configured but waiting** (audio can't autoplay) — the terminal prints `shared session: box · 5m · rain — press any key (or tap) to begin`, the orb breathes silently, and the gesture starts audio. The boot sequence is skipped on a deep-link (straight to the configured session). An invalid hash (`#p=unknown`) → a dim `unknown pattern 'unknown' — starting calm` and falls back to defaults.
**Patterns to follow:** versioned-storage / single-initializer pattern; `src/state.rs` (parity of what's remembered); `src/streak.rs` (streak math parity).
**Test scenarios:**
- Vitest: streak derivation (consecutive incl. today; a gap resets the streak but the 30-day rate persists; local-midnight boundary) — **parity** with `src/streak.rs`.
- Vitest: the initializer upgrades an absent/corrupt blob to current with no data loss; `export`→`import` round-trips.
- Vitest: deep-link encode/decode round-trips `{pattern,for,sound}`; a malformed/unknown hash falls back to defaults with a message.
**Verification:** Streak persists + renders as a GitHub-style heatmap; a shared `#…` link lands pre-configured-and-waiting, then one gesture starts it; export/import works.

---

### Phase C — Ship

### U8. Subtle nudges, the boot showpiece, and GitHub Pages deploy

**Goal:** The terminal-native install/Pilgrim discovery, a designed boot moment, and the live site.
**Requirements:** R6, R7, R9 (boot/reduced-motion), success criteria.
**Dependencies:** U3–U7.
**Files:** `web/src/{boot.ts}`; `web/src/commands/{install.ts, whoami.ts, man.ts}`; `.github/workflows/pages.yml`; `README.md`.
**Approach:**
- **Boot sequence (spec'd, anti-slop):** after WASM/font are ready (the U3 placeholder covers init), a ~1s login-style sequence — `Last login: <relative time> on cli.pilgrimapp.org`, a one-line dim `meditate <version>`, then the orb fades in and the MOTD prints. **Any keypress/tap skips** to the MOTD. `prefers-reduced-motion` → skip straight to the final frame (no scanline). No Matrix-rain / generic effects.
- **Nudges as discoveries:** `install`/`brew` print the exact brew line; copy-on-click copies **only the visible text** (a deliberate anti-malware trust signal); `which meditate` → `not installed — run 'install' to get the real thing`. Pilgrim is a whisper only in `whoami`/`credits` (`a breather · made by the folks behind Pilgrim — pilgrimapp.org`). `help` lists only the obvious commands (install/whoami stay soft discovery).
- **Deploy:** `pages.yml` installs the wasm toolchain (`rustup target add wasm32-unknown-unknown`, `cargo binstall wasm-bindgen-cli@<pinned>`, `wasm-opt`), runs `web/scripts/build-wasm.sh` + `npm ci && npm run build`, then `upload-pages-artifact` (`web/dist`) + `deploy-pages`. `README.md` gains a **"▶ Try it live"** button.
- **Documented ops (not code):** set the Pages custom domain + Enforce HTTPS in Settings; configure R2 CORS (`Allow-Origin: https://cli.pilgrimapp.org`, `GET`/`HEAD`) and **purge the Cloudflare cache**.
**Patterns to follow:** the existing `.github/workflows/release.yml` Actions conventions; terminal.shop restraint; the research copy-verbatim security note.
**Test scenarios:**
- Vitest: `install` output contains the exact brew line and nothing hidden; the copy handler copies exactly the displayed string (no appended bytes).
- Vitest: `whoami` surfaces the Pilgrim + repo hint; `help` does not; `prefers-reduced-motion` skips the boot scanline to the final frame.
- (CI) `pages.yml` builds the wasm + site → `web/dist`; (manual, post-DNS) HTTPS at `cli.pilgrimapp.org`, audio loads (CORS OK).
**Verification:** Live at `cli.pilgrimapp.org`; the boot is a designed login moment (skippable); `install` copies the verbatim brew line; Pilgrim is a whisper; README has the "Try it live" button.

---

## Scope Boundaries

**Deferred for later (aligned, not v1):** the embeddable "breathe break" web component (origin R12); full pack-browsing UI; a v2 persistence migration ladder (only when a v2 schema exists).

**Outside this product's identity (won't build):** accounts/login/cloud-sync/cross-device; co-breathing / sync rooms (need a server); any telemetry/analytics; marketing chrome (popups, modal banners, email capture).

**Deferred to Follow-Up Work (plan-local sequencing):** a Playwright end-to-end pass (browser-render/audio/canvas can't be unit-tested); moving the CLI package itself into `crates/meditate-cli/` (kept at root in v1 to avoid release-pipeline churn). **Phase A (U1–U3) ships as a deployable checkpoint** before Phase B/C — it satisfies the primary "breathe within ~3s, no install" criterion and de-risks the U1 refactor + the WASM/xterm/iOS-audio stack on real devices before the rest is built on top.

---

## Dependencies / Prerequisites

- **Ops (user-controlled):** R2 bucket CORS for `cli.pilgrimapp.org` + Cloudflare cache purge; DNS CNAME `cli` → `<org>.github.io`; set the Pages custom domain + Enforce HTTPS in Settings.
- **Toolchain:** `wasm32-unknown-unknown`, `wasm-bindgen-cli` (pinned to the lib version, one source), `wasm-opt`, Node 20, a committed `web/package-lock.json` (`npm ci`).
- **Confirmed:** the CDN is live with the documented manifest/URL schema; the pure core is dep-free after the U1 decoupling.

---

## Risks & Mitigations

- **U1 cross-crate edges (palette→clap, render→term)** → handled explicitly in U1 (core `caps` module + clap-free `Pin` + CLI re-exports); the full CLI suite + a wasm32 build both gate U1.
- **WASM/native divergence in core** → `ci.yml` gains a wasm32 build on every PR (U1/U2), so a `std::env`/`Instant` in core fails on the PR, not at deploy.
- **wasm-bindgen lib/CLI skew** → both pinned from one source; the build script + CI fail on mismatch; install the CLI via `cargo binstall <exact>`.
- **Two-toolchain maintenance** (counter to the repo's pinning discipline) → commit `web/package-lock.json`, require `npm ci`, and document the JS-tree update cadence; the wasm-bindgen pin is the one cross-ecosystem coupling and CI asserts it.
- **Parity drift** (REPL/audio/streak/deep-links are new TS) → explicit parity-by-test scenarios in U4/U5/U7 against `src/cli.rs`, `src/audio/mod.rs`, `src/streak.rs`.
- **iOS audio (silent fail, mute switch)** → gesture-gate + warm-buffer + verify `ctx.state`; designed "audio unavailable / system muted" states; real-hardware test.
- **CORS misconfig** → loud failure (`decodeAudioData` rejects) → the designed fallback; document exact R2 headers + cache-purge.

---

## Success Criteria (from origin, reconciled with autoplay reality)

- The **visual** orb breathes within ~3s of landing; **sound on the first interaction** (the gesture-unlock, made obvious by the first-paint affordance) — no install, no account.
- The same commands work on web and CLI, kept in parity by test (the site teaches the CLI).
- The install nudge is discoverable but never reads as an ad.
- Beautiful enough to earn an unsolicited "whoa" — the boot, the iOS-accurate smooth orb, and the breathing tab.
- Zero backend, zero account, zero telemetry — identity intact.
