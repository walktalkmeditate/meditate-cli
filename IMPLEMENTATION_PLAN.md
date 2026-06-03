# Graphics Orb — Implementation Plan

A real, anti-aliased orb drawn via terminal inline-graphics protocols, with a
clean fallback to today's half-block orb everywhere else.

**Targets (locked):** the kitty graphics protocol — which covers **kitty,
Ghostty, and WezTerm** — plus **iTerm2** (OSC 1337). Sixel deferred. Alacritty
and everything else fall back to the block orb.

**Key existing seams:** `OrbScene` + `orb::paint(surface, scene)` is
resolution-independent, so a graphics renderer just paints into a higher-res
`Surface` and emits it as an image. `term::GraphicsProtocol` + `detect_graphics`
already exist. `render::renderer_for(caps)` is where a graphics renderer is chosen.

## Stage 1: Detection
**Goal**: Recognize every kitty-protocol terminal, including Ghostty.
**Success Criteria**: `detect_graphics` returns `Kitty` for kitty, Ghostty
(`TERM=xterm-ghostty` / `TERM_PROGRAM=ghostty`), and WezTerm; `ITerm2` for
iTerm2; `None` otherwise.
**Tests**: `detect_graphics` truth table via `MapEnv`.
**Status**: Complete

## Stage 2: Kitty RGBA renderer (the wow — kitty / Ghostty / WezTerm)
**Goal**: Draw the orb as a real image over the kitty graphics protocol.
**Success Criteria**:
- rasterize `OrbScene` to a high-res RGBA buffer (smooth radial glow,
  anti-aliased edge, palette color, breath scale, ripple rings)
- emit kitty escapes: transmit RGBA (chunked base64) + place, deleting the prior
  placement each frame so frames don't stack
- `--no-graphics` flag + `graphics` config; `renderer_for`/session pick the
  graphics path when the protocol is `Kitty`, it's enabled, and we're not in a
  non-passthrough tmux; any failure falls back to the block orb
- the status line still renders below the image
**Tests**: encoder unit tests (header keys, base64 payload, delete-then-place
order) on a small buffer; PTY check that valid `_G` escapes are emitted; visual
confirmation by the user in kitty/Ghostty.
**Status**: Not Started

## Stage 3: iTerm2 (OSC 1337) path
**Goal**: The same orb on iTerm2.
**Success Criteria**: minimal PNG encode of the RGBA buffer; emit the OSC 1337
inline-image envelope sized to the orb region; selected when protocol is `ITerm2`.
**Tests**: PNG header/IHDR sanity; OSC 1337 envelope; PTY emit check.
**Status**: Not Started

## Stage 4: Polish + docs
**Goal**: Make it feel great and document it.
**Success Criteria**: cell-pixel sizing (query `CSI 14 t`, sane default when
unanswered); 30fps perf check; tmux/ssh fallback; clean teardown (delete images
on exit, like the title guard); README + config template document `--no-graphics`.
**Tests**: sizing fallback; teardown emits delete-all.
**Status**: Not Started

## Execution notes / unknowns
- Cell pixel size: query `CSI 14 t` (text-area px) or `CSI 16 t` (cell px); fall
  back to ~8×16 when the terminal doesn't answer.
- Per-frame transmit: a ~300px RGBA orb is ~360KB raw → ~480KB base64; fine over
  a local PTY at the orb's phase-dependent frame rate. Reuse one image id and
  delete the prior placement so frames don't accumulate.
- kitty takes raw RGBA (`f=32`) with no dependency; iTerm2 needs a container, so
  Stage 3 adds minimal PNG encoding.
- Rendering can only be *visually* verified in a real kitty/Ghostty/iTerm2 —
  automated tests confirm valid escapes; the look is the user's eyeball check.
