import type { Session } from '../wasm/meditate_wasm.js';
import type { Terminal } from '@xterm/xterm';
import type { AudioEngine } from '../audio';
import type { Persistence } from '../store';
import type { DeepLink } from '../deeplink';

/** Services a command can use. */
export interface CommandContext {
  session: Session;
  term: Terminal;
  audio: AudioEngine;
  store: Persistence;
  version: string;
  /** Show a full-screen page; the orb pauses until the user presses a key. */
  page: (text: string) => void;
  /** A transient one-line confirmation, shown briefly above the prompt. */
  status: (line: string) => void;
  /** The active pattern name (the façade tracks phase, not pattern). */
  currentPattern: () => string;
  /** Switch breathing pattern and confirm (keeps pattern logic in one place). */
  setPattern: (name: string) => void;
  /** Whether the smooth Canvas-2D orb is showing (vs the half-block orb). */
  graphicsMode: () => boolean;
  setGraphics: (smooth: boolean) => void;
  /** Track the active soundscape (for the share link + persistence). */
  setSound: (id: string | null) => void;
  /** The current session config, for building a share link. */
  shareLink: () => DeepLink;
  /** Non-hidden commands, for `help` and `man` (kept honest as units land). */
  visibleCommands: () => Command[];
}

export interface Command {
  name: string;
  aliases?: string[];
  summary: string;
  /** Hidden from `help` — the soft-discovery commands (install, whoami). */
  hidden?: boolean;
  run(args: string[], ctx: CommandContext): void | Promise<void>;
}

export interface Parsed {
  name: string;
  args: string[];
}

/** Split a command line into a lowercased verb and its raw args. */
export function parseLine(line: string): Parsed | null {
  const parts = line.trim().split(/\s+/).filter(Boolean);
  if (parts.length === 0) return null;
  return { name: parts[0].toLowerCase(), args: parts.slice(1) };
}
