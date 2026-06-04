// Tiny ANSI helpers for command output. Moss is meditate's accent — the orb's
// core color — so the terminal copy feels of a piece with the breathing orb.

const RESET = '\x1b[0m';

export const moss = (s: string): string => `\x1b[38;2;134;184;144m${s}${RESET}`;
export const dim = (s: string): string => `\x1b[2m${s}${RESET}`;
export const bold = (s: string): string => `\x1b[1m${s}${RESET}`;

/** Pad a string to `width` on the right (for aligned columns). */
export const pad = (s: string, width: number): string =>
  s.length >= width ? s : s + ' '.repeat(width - s.length);
