---
date: 2026-06-02
topic: meditate-cli
---

# meditate — a terminal breathing companion

## Summary

A free, MIT-licensed CLI called `meditate` that turns the terminal into a glanceable breathing companion you can run *during* a meeting. It ports Pilgrim's 7 breathing patterns and its moss-orb animation to the terminal, with three opt-in sound layers (soundscapes, voice guides, bells) switched live by single keystroke, plus local streaks, opt-in workflow nudges, and a seasonal palette. It works instantly with zero downloads and never phones home.

---

## Problem Frame

The most common moment of stress for a terminal-dwelling developer isn't a crisis — it's the mid-day drag. A meeting that's run long, attention fraying, the urge to do *something* with the restlessness. The phone is the usual escape, but reaching for it pulls you out of the room and into a different attention economy. Existing terminal options (`breathe-cli` and friends) prove the appetite — a breathing tool living where the work already happens resonated enough to climb Hacker News — but they're minimal: a plain animation, no sound, no patterns to choose from, no craft.

Pilgrim already has the substance — seven tuned breathing patterns, a genuinely beautiful breathing animation, voice guides, soundscapes, bells — but it lives on a phone, behind a context switch, and is built for eyes-closed, full-attention, walking meditation. None of that fits the dev who needs to breathe through the next twenty minutes of a call without leaving their seat or their screen.

---

## Actors

- A1. Terminal-dwelling developer: the primary user. Runs `meditate` for a mid-meeting reset or a quick breather, often half-present and eyes-open. Wants instant, beautiful, low-demand.
- A2. The `meditate` CLI: renders the breathing experience, manages the three sound layers and local state, and runs fully offline by default.
- A3. Pilgrim CDN: optional, user-triggered source of soundscape / voice-guide / bell packs. Never contacted without an explicit user action.
- A4. Pilgrim iOS app: the downstream destination the soft "door" points toward. Referenced, not integrated.

---

## Key Flows

- F1. Mid-meeting breath (the core moment)
  - **Trigger:** User types `meditate` during a long call.
  - **Actors:** A1, A2
  - **Steps:** App launches full-screen in ~1s on the last-used pattern → orb breathes in time → user optionally pulls up a soundscape with one keystroke (headphones) → breathes along while half-attending the meeting → presses `q` when the call picks back up.
  - **Outcome:** User regulated through the moment without leaving the terminal, making noise they didn't choose, or context-switching to a phone.
  - **Covered by:** R1, R3, R6, R8, R9

- F2. First run, nothing downloaded (offline)
  - **Trigger:** Fresh install, no packs, possibly no network.
  - **Actors:** A1, A2
  - **Steps:** `meditate` launches → breath + orb + built-in bell work fully → a calm, non-blocking hint mentions `meditate download` for soundscapes/voices/bells when wanted.
  - **Outcome:** A complete, beautiful experience with zero downloads and zero network calls.
  - **Covered by:** R1, R10, R11

- F3. Stepped-away guided session (voice)
  - **Trigger:** User has a real 5–15 min break and wants guidance, enables a voice guide.
  - **Actors:** A1, A2, A3
  - **Steps:** User downloads a voice pack once → starts a session with voice on → phase-appropriate prompts arrive (settling → deepening → closing) while the soundscape ducks beneath the voice → session closes with a bell.
  - **Outcome:** A deeper, guided sit for the moments when full attention *is* available.
  - **Covered by:** R10, R12, R13

- F4. Workflow nudge (opt-in)
  - **Trigger:** User has installed the shell/git/tmux integration; a long-running command just finished.
  - **Actors:** A1, A2
  - **Steps:** Integration surfaces a quiet, dismissible offer to breathe → user accepts (launches a quick session) or ignores it.
  - **Outcome:** Meditation woven into the dev day at natural seams, never forced.
  - **Covered by:** R15

---

## Requirements

**Core breathing experience**
- R1. `meditate` launches straight into a full-screen breathing session in roughly a second, with no login, setup, or menu; the last-used pattern resumes by default.
- R2. Ships 7 breathing patterns ported from Pilgrim, each selectable by name or cycle key.

  | Pattern | Label | Inhale | Hold | Exhale | Hold | Note |
  |---|---|---|---|---|---|---|
  | Calm | 5 / 7 | 5 | — | 7 | — | Long exhale, gentle relaxation |
  | Equal | 4 / 4 | 4 | — | 4 | — | Balanced and simple |
  | Relaxing | 4-7-8 | 4 | 7 | 8 | — | Deep relaxation with held breath |
  | Box | 4-4-4-4 | 4 | 4 | 4 | 4 | Four equal phases for focus |
  | Coherent | 5 / 5 | 5 | — | 5 | — | Heart-rate-variability training |
  | Deep calm | 3 / 6 | 3 | — | 6 | — | Short inhale, slow release |
  | None | — | 0 | 0 | 0 | 0 | Still focus point, open meditation |

  *Source of truth for timings and behavior: `../pilgrim-ios/Pilgrim/Scenes/ActiveWalk/MeditationView.swift` (`BreathRhythm.all`).*
- R3. The orb animates in time with the active pattern — expanding on inhale, holding with an intensified inner glow during holds, contracting on exhale — with smooth easing, and emits an outward ripple ring on each inhale.
- R4. The orb renders as a smooth radial gradient (moss → parchment), using the richest visuals the terminal supports and degrading gracefully through reduced-color and reduced-motion fallbacks; it scales live to the terminal size, down to a small tmux pane.
- R5. A soft phase cue ("inhale / hold / exhale") and a breath count are shown unobtrusively; milestone flashes mark the 5 / 10 / 15-minute points.
- R6. Sessions run open-ended by default (until the user quits); an optional timed or breath-count mode ends with a soft bell.
- R7. Quitting fades the orb out gracefully rather than hard-clearing the screen.

**Live keyboard control**
- R8. All controls are single, discoverable keystrokes with an auto-hiding hint bar, and never interrupt the breath. At minimum: switch pattern, cycle soundscape, cycle/toggle voice, toggle bell, mute all, volume up/down, pause, focus (hide chrome), and quit.
- R9. Each of the three sound layers can be switched and independently turned off live, without restarting the session.

**Sound layers (opt-in)**
- R10. Three sound layers are available — soundscapes, voice guides, and bells — each sourced as an optional pack the user explicitly downloads; none is required to use the app.
- R11. On first run with nothing downloaded, breath + visuals (plus a built-in start/stop bell) work fully offline; a clear, non-blocking command lets users fetch packs when they want them.
- R12. Soundscapes loop seamlessly and crossfade when switched; a playing voice guide ducks the soundscape beneath it.
- R13. Voice guides deliver only Pilgrim's *meditation* prompts — the phase-appropriate set (settling → deepening → closing) with its meditation scheduling — and never walk-context narration; they are off by default. (Parity anchor: each voice pack's `meditationPrompts` / `meditationScheduling`, not its walk prompts.)

**Ritual, nudges, palette (v1 extras)**
- R14. A local-only, file-based record tracks total minutes and a daily streak with no account and no network; it surfaces a gentle line on launch and can be fully disabled.
- R15. Opt-in shell / git / tmux integrations can offer a breath at natural seams (e.g., after a long-running command); they are inert until the user installs them and never force an interruption.
- R16. The orb palette shifts with season and time of day (dawn / day / dusk / night), ported from Pilgrim's seasonal color system, and can be pinned to a fixed palette.

**The Pilgrim door**
- R17. On a long-session exit, a single tasteful, dismissible line invites continuing with the Pilgrim app's walking meditations; it can be turned off and never tracks the user.

**Distribution & licensing**
- R18. Installable via Homebrew plus at least one one-line method for every major platform (macOS, Linux, Windows), with no runtime account or required configuration.
- R19. Licensed MIT, with a polished `--help` and a README that shows the experience (e.g., an animated capture).

---

## Acceptance Examples

- AE1. **Covers R1, R11.** Given a fresh install with no packs downloaded and no network connection, when the user runs `meditate`, then a full breathing session starts within ~1s with orb, paced breathing, and a built-in bell, and nothing is fetched over the network.
- AE2. **Covers R9, R12.** Given a session with a soundscape playing, when the user enables a voice guide via its key, then the soundscape volume ducks beneath the voice; when the user toggles the voice back off, the soundscape returns to full and the session never restarts.
- AE3. **Covers R4.** Given a terminal lacking truecolor or inline-graphics support (or with reduce-motion enabled), when a session runs, then the orb still renders legibly via lower-fidelity drawing (and/or reduced motion) without errors.
- AE4. **Covers R10, R17.** Given the user has never downloaded a pack and keeps the Pilgrim door enabled, when they end a long session, then a single dismissible line invites them to Pilgrim — and no network call or tracking event occurs as a result.
- AE5. **Covers R14.** Given the local record is enabled and the user has meditated three days running, when they launch on the fourth day, then a gentle streak line appears; when the record is disabled, no such line appears and no file is read or written.
- AE6. **Covers R15.** Given the shell integration is not installed, when a long command finishes, then nothing happens; given it is installed, the same event surfaces a quiet, dismissible offer to breathe.
- AE7. **Covers R13.** Given a downloaded voice pack containing both walk and meditation prompts, when a guided session runs, then only the pack's meditation prompts play (on its meditation schedule) and no walk-context narration is ever selected.

---

## Success Criteria

- A stressed developer can go from typing `meditate` to a calmer breath in seconds, mid-meeting, without it pulling their focus, making noise they didn't choose, or requiring a phone.
- The visual is good enough that people screenshot or record it unprompted — craft is the growth engine.
- Everything works on first run with zero downloads and zero network calls; downloads are always user-initiated and obvious.
- `ce-plan` can build v1 without inventing product behavior: the patterns, controls, sound model, offline-first rule, extras, and distribution targets are all specified here.

---

## Scope Boundaries

### Deferred for later

- Stealth / meeting-disguise mode (a look that hides what it is from a glancing screen-share). Considered and explicitly set aside for v1.
- Biofeedback or heart-rate-synced breathing.
- User-authored custom breathing patterns beyond the shipped 7.
- Mixing two soundscapes simultaneously (e.g., rain + fire).

### Outside this product's identity

- Accounts, cloud sync, or cross-device history — it is account-free by design.
- Monetization, a paid tier, or paywalled packs — it stays a free MIT gift.
- Telemetry, analytics, or any usage tracking.
- Becoming a full guided-meditation course library — it is a breathing *companion*, not a content platform.

---

## Key Decisions

- Companion over faithful port: the spine is the glanceable, eyes-open, runs-forever mid-meeting experience, not a 1:1 recreation of the iOS app's eyes-closed closing ceremony. The iOS app sets the *craft bar* for visuals, not the interaction model.
- Offline-first with opt-in downloads: core breath + visuals ship inside the binary and work with no network; the three sound layers are packs the user explicitly pulls. This is what keeps "zero phone-home," "instant-on mid-meeting," and "include every sound layer" simultaneously true.
- "Zero phone-home" defined: no telemetry and no background calls of any kind; the only network activity is a pack download the user explicitly triggers.
- Voice guides as the secondary layer: the live-meeting core is breath + sound; a narrating voice serves the deeper "stepped-away" moment and is off by default.
- Reuse Pilgrim's CDN packs and seasonal palette: leverages existing tuned assets and brand DNA rather than building a new content pipeline.
- Pilgrim CDN packs cleared for reuse: all three pack types (soundscapes, voices, bells) can be served to the CLI directly from Pilgrim's existing CDN manifests.
- Voice guides use Pilgrim's meditation prompts only: as a breathing/meditation tool, the CLI ignores walk-context prompts entirely and pulls only the meditation prompt set and its scheduling from each voice pack.
- MIT, no account, no tracking: a genuine gift posture that doubles as a soft, classy front door to Pilgrim.

---

## Dependencies / Assumptions

- Pilgrim's CDN manifests for voice guides, soundscapes, and bells (e.g., `cdn.pilgrimapp.org/voiceguide/...`, `/audio/...`) are cleared for reuse by the CLI. *Confirmed — the user controls the Pilgrim ecosystem.*
- Reliable terminal capability detection (color depth, inline-graphics support, window size, reduce-motion) is achievable across common terminals. *Assumption — to validate in planning.*
- Cross-platform audio playback from a CLI (macOS / Linux / Windows) is feasible and acceptable for soundscapes, voices, and bells. *Assumption — to validate in planning.*
- The 7 breathing patterns and animation behavior in `../pilgrim-ios/Pilgrim/Scenes/ActiveWalk/MeditationView.swift` are the canonical reference for parity.

---

## Outstanding Questions

### Deferred to Planning

- [Affects R4][Technical] Which rendering tiers to target (inline graphics vs. half-block/braille vs. 16-color) and how to detect terminal capability.
- [Affects R10–R13][Technical] Cross-platform audio playback approach, and pack format / download / caching / integrity.
- [Affects R13][Needs research] Voice-guide prompt scheduling parity with iOS (density, spacing, phase windows).
- [Affects R18][Technical] Concrete packaging targets and one-line installers per platform (Homebrew, and the equivalents for Linux/Windows).
