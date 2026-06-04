import { PATTERNS, isPattern, PATTERN_LABELS } from '../patterns';
import { renderMotd } from '../motd';
import type { Command, CommandContext } from './types';
import { parseLine } from './types';
import { helpCommand, manCommand } from './help';
import { installCommand, whichCommand, whoamiCommand } from './discovery';

export type { Command, CommandContext } from './types';

const pauseCommand: Command = {
  name: 'pause',
  aliases: ['resume'],
  summary: 'freeze / resume the breath',
  run: (_args, ctx) => {
    ctx.session.pauseToggle();
    ctx.status(ctx.session.isPaused() ? 'paused' : 'resumed');
  },
};

const clearCommand: Command = {
  name: 'clear',
  summary: 'redraw a clean screen',
  run: (_args, ctx) => {
    ctx.term.clear();
    ctx.status('');
  },
};

const graphicsCommand: Command = {
  name: 'graphics',
  aliases: ['orb'],
  summary: 'toggle the smooth orb',
  run: (_args, ctx) => {
    const smooth = !ctx.graphicsMode();
    ctx.setGraphics(smooth);
    ctx.status(smooth ? 'graphics: smooth' : 'graphics: blocks');
  },
};

function cyclePattern(ctx: CommandContext, dir: number): void {
  const i = PATTERNS.indexOf(ctx.currentPattern() as (typeof PATTERNS)[number]);
  const next = PATTERNS[(((i < 0 ? 0 : i) + dir) % PATTERNS.length + PATTERNS.length) % PATTERNS.length];
  ctx.setPattern(next);
}

const nextCommand: Command = {
  name: 'next',
  summary: 'next pattern',
  run: (_args, ctx) => cyclePattern(ctx, 1),
};

const prevCommand: Command = {
  name: 'prev',
  summary: 'previous pattern',
  run: (_args, ctx) => cyclePattern(ctx, -1),
};

const motdCommand: Command = {
  name: 'motd',
  summary: 'the welcome banner',
  hidden: true,
  run: (_args, ctx) => ctx.page(renderMotd(ctx.version)),
};

export interface Registry {
  list: Command[];
  map: Map<string, Command>;
}

/** Build the command registry. Later units pass their commands via `extra`. */
export function buildRegistry(extra: Command[] = []): Registry {
  const list: Command[] = [
    pauseCommand,
    nextCommand,
    prevCommand,
    ...extra,
    graphicsCommand,
    clearCommand,
    helpCommand,
    manCommand,
    installCommand,
    whichCommand,
    whoamiCommand,
    motdCommand,
  ];
  const map = new Map<string, Command>();
  for (const c of list) {
    map.set(c.name, c);
    for (const a of c.aliases ?? []) map.set(a, c);
  }
  return { list, map };
}

/** Parse and run one command line. Bare pattern names switch the breath. */
export function runCommand(line: string, registry: Registry, ctx: CommandContext): void | Promise<void> {
  const parsed = parseLine(line);
  if (!parsed) return;
  const { name, args } = parsed;

  if (isPattern(name)) {
    ctx.setPattern(name);
    return;
  }
  if (name === 'meditate') {
    if (args.length === 0) {
      ctx.status('already breathing — type a pattern name, or `help`');
      return;
    }
    const candidate = args[0].toLowerCase();
    if (isPattern(candidate)) {
      ctx.setPattern(candidate);
      return;
    }
    ctx.status(`unknown pattern '${candidate}'`);
    return;
  }

  const cmd = registry.map.get(name);
  if (!cmd) {
    ctx.status(`unknown command '${name}' — type 'help'`);
    return;
  }
  return cmd.run(args, ctx);
}

/** A short, friendly status line for a pattern switch (e.g. "box · 4-4-4-4"). */
export function patternStatus(name: string): string {
  const label = PATTERN_LABELS[name as (typeof PATTERNS)[number]];
  return label ? `${name} · ${label}` : name;
}
