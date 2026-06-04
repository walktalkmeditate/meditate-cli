import type { Terminal } from '@xterm/xterm';
import type { FitAddon } from '@xterm/addon-fit';
import type { Session } from './wasm/meditate_wasm.js';

// ── Pure helpers (unit-tested; no DOM, no wasm) ──────────────────────────────

/** Begin/end Synchronized Output: the terminal buffers the whole frame, then
 *  swaps it in one paint, so the orb never tears mid-write. */
const BSU = '\x1b[?2026h';
const ESU = '\x1b[?2026l';
/** Home the cursor and overwrite in place — never clear-screen (`\x1b[2J`),
 *  which blanks the grid for a frame and reads as a flash. */
const HOME = '\x1b[H';

/**
 * Wrap one frame's content for a tear-free, flash-free write: synchronized
 * output around a cursor-home overwrite. Deliberately never emits `\x1b[2J`.
 */
export function frameSequence(content: string): string {
  return BSU + HOME + content + ESU;
}

/** Frame throttle: render only once `minInterval` ms have passed since `last`.
 *  `last < 0` means "never drawn", so the first frame always draws. */
export function shouldDraw(last: number, now: number, minInterval: number): boolean {
  return last < 0 || now - last >= minInterval;
}

/** Center a single line within `cols`, clipping if it is too wide. */
export function centerLine(text: string, cols: number): string {
  if (text.length >= cols) return text.slice(0, cols);
  const pad = Math.floor((cols - text.length) / 2);
  return ' '.repeat(pad) + text;
}

const FPS_NORMAL = 30;
const FPS_REDUCED = 12;

export interface LoopOptions {
  term: Terminal;
  fit: FitAddon;
  session: Session;
  reduceMotion: boolean;
  /** A dim status/hint line for the bottom row, or `null` for none. */
  hint: () => string | null;
}

export interface LoopHandle {
  stop(): void;
}

/**
 * Drive the breath: one `requestAnimationFrame` loop, throttled, that asks the
 * wasm session for the orb ANSI and writes exactly one synchronized frame. The
 * session is driven by absolute elapsed time, so a backgrounded tab (where rAF
 * is paused) simply resumes at the correct breath phase — no drift to clamp.
 */
export function startBreathing(opts: LoopOptions): LoopHandle {
  const minInterval = 1000 / (opts.reduceMotion ? FPS_REDUCED : FPS_NORMAL);
  let raf = 0;
  let startedAt = -1;
  let lastDraw = -1;

  const frame = (t: number) => {
    raf = requestAnimationFrame(frame);
    if (startedAt < 0) startedAt = t;
    if (!shouldDraw(lastDraw, t, minInterval)) return;
    lastDraw = t;

    const { cols, rows } = opts.term;
    if (cols === 0 || rows === 0) return;

    const orbRows = Math.max(1, rows - 1);
    const orb = opts.session.tickFrame(t - startedAt, cols, orbRows);

    const hint = opts.hint();
    const tail =
      hint !== null ? `\r\n\x1b[2m${centerLine(hint, cols)}\x1b[0m` : '';

    opts.term.write(frameSequence(orb + tail));
  };

  raf = requestAnimationFrame(frame);
  return {
    stop() {
      cancelAnimationFrame(raf);
    },
  };
}
