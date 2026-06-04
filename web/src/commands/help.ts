import { bold, dim, moss, pad } from '../ansi';
import { PATTERNS, PATTERN_LABELS } from '../patterns';
import type { Command, CommandContext } from './types';

const PATTERN_BLURBS: Record<string, string> = {
  calm: 'a long, settling exhale (the default)',
  equal: 'balanced, even breathing',
  relaxing: 'the classic unwind',
  box: 'steady four-count box breathing',
  coherent: '~6 breaths a minute',
  'deep-calm': 'a gentle, slowing rhythm',
  none: 'hold a single still point',
};

function commandList(ctx: CommandContext): string {
  return ctx
    .visibleCommands()
    .map((c) => `  ${moss(pad(c.name, 12))} ${dim(c.summary)}`)
    .join('\n');
}

/** The short list — what `help` shows. */
export function helpPage(ctx: CommandContext): string {
  return [
    bold('commands'),
    commandList(ctx),
    '',
    dim("type a pattern name to switch · `man` for the full guide · `install` to run it for real"),
  ].join('\n');
}

/** The full reference — what `man` shows. The command section is generated from
 *  the live registry so it stays honest as units land. */
export function manPage(ctx: CommandContext): string {
  const patterns = PATTERNS.map(
    (p) => `  ${moss(pad(p, 12))}${dim(pad(PATTERN_LABELS[p], 10))}${PATTERN_BLURBS[p]}`,
  ).join('\n');

  return [
    bold('meditate') + dim(` — a terminal breathing companion · v${ctx.version}`),
    '',
    '  A quiet place to breathe. Free, no account — nothing leaves your browser.',
    '  This web terminal mirrors the meditate CLI: the same commands work in your',
    '  real terminal once you install it.',
    '',
    bold('patterns'),
    patterns,
    '',
    dim('  switch any time by typing its name (e.g. `box`), or `next` / `prev`.'),
    '',
    bold('commands'),
    commandList(ctx),
    '',
    bold('keys'),
    dim('  enter runs a command · ↑ ↓ walk history · tab completes'),
    '',
    dim('  a soft companion to Pilgrim, a walking-meditation app — type `whoami`.'),
  ].join('\n');
}

export const helpCommand: Command = {
  name: 'help',
  aliases: ['?'],
  summary: 'the command list',
  run: (_args, ctx) => ctx.page(helpPage(ctx)),
};

export const manCommand: Command = {
  name: 'man',
  summary: 'the full guide',
  run: (_args, ctx) => ctx.page(manPage(ctx)),
};
