// The breathing patterns, mirroring src/breath.rs PATTERNS and the CLI's
// PatternName (src/cli.rs). Parity is asserted in commands.test.ts — the web
// parser must accept exactly the patterns the CLI accepts, no more, no less.

export const PATTERNS = [
  'calm',
  'equal',
  'relaxing',
  'box',
  'coherent',
  'deep-calm',
  'none',
] as const;

export type PatternName = (typeof PATTERNS)[number];

/** Cadence labels, mirroring breath.rs `Pattern.label`. */
export const PATTERN_LABELS: Record<PatternName, string> = {
  calm: '5 / 7',
  equal: '4 / 4',
  relaxing: '4-7-8',
  box: '4-4-4-4',
  coherent: '5 / 5',
  'deep-calm': '3 / 6',
  none: 'open',
};

export function isPattern(name: string): name is PatternName {
  return (PATTERNS as readonly string[]).includes(name);
}
