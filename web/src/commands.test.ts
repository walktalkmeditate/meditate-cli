import { describe, it, expect, vi } from 'vitest';
import { PATTERNS, isPattern } from './patterns';
import { parseLine } from './commands/types';
import { buildRegistry, runCommand } from './commands';
import type { CommandContext, AppearanceMode } from './commands';

// Parity guard: the web parser must accept exactly the patterns the CLI accepts
// (src/cli.rs PatternName -> as_str). If the CLI gains a pattern, this fails.
const CLI_PATTERNS = ['calm', 'equal', 'relaxing', 'box', 'coherent', 'deep-calm', 'none'];

describe('pattern parity with the CLI', () => {
  it('accepts exactly the CLI patterns, no more, no less', () => {
    expect([...PATTERNS].sort()).toEqual([...CLI_PATTERNS].sort());
    for (const p of CLI_PATTERNS) expect(isPattern(p)).toBe(true);
    expect(isPattern('square')).toBe(false);
    expect(isPattern('')).toBe(false);
  });
});

describe('parseLine', () => {
  it('splits a verb and args and lowercases the verb', () => {
    expect(parseLine('  Sound   Forest  ')).toEqual({ name: 'sound', args: ['Forest'] });
    expect(parseLine('box')).toEqual({ name: 'box', args: [] });
    expect(parseLine('   ')).toBeNull();
  });
});

function makeContext() {
  const session = {
    setPattern: vi.fn(),
    pauseToggle: vi.fn(),
    isPaused: vi.fn(() => false),
  };
  const calls = { page: vi.fn(), status: vi.fn(), setPattern: vi.fn(), setAppearance: vi.fn() };
  let current = 'calm';
  let appearanceMode: AppearanceMode = 'auto';
  const registry = buildRegistry();
  const ctx: CommandContext = {
    // Only the methods the commands under test touch are real.
    session: session as unknown as CommandContext['session'],
    term: { clear: vi.fn() } as unknown as CommandContext['term'],
    audio: {} as unknown as CommandContext['audio'],
    store: {} as unknown as CommandContext['store'],
    version: '0.0.0',
    page: calls.page,
    status: calls.status,
    currentPattern: () => current,
    setPattern: (name) => {
      current = name;
      calls.setPattern(name);
    },
    graphicsMode: () => false,
    setGraphics: vi.fn(),
    appearance: () => appearanceMode,
    setAppearance: (mode) => {
      appearanceMode = mode;
      calls.setAppearance(mode);
    },
    setSound: vi.fn(),
    shareLink: () => ({ pattern: current }),
    visibleCommands: () => registry.list.filter((c) => !c.hidden),
  };
  return { ctx, registry, calls, session };
}

describe('runCommand dispatch', () => {
  it('switches the breath on a bare pattern name', () => {
    const { ctx, registry, calls } = makeContext();
    runCommand('box', registry, ctx);
    expect(calls.setPattern).toHaveBeenCalledWith('box');
  });

  it('accepts `meditate <pattern>` and rejects an unknown pattern', () => {
    const { ctx, registry, calls } = makeContext();
    runCommand('meditate relaxing', registry, ctx);
    expect(calls.setPattern).toHaveBeenCalledWith('relaxing');

    runCommand('meditate sideways', registry, ctx);
    expect(calls.status).toHaveBeenCalledWith(expect.stringContaining('unknown pattern'));
  });

  it('accepts a capitalized `meditate <Pattern>` (case-insensitive like the bare form)', () => {
    const { ctx, registry, calls } = makeContext();
    runCommand('meditate Box', registry, ctx);
    expect(calls.setPattern).toHaveBeenCalledWith('box');
  });

  it('toggles pause and reports an unknown command', () => {
    const { ctx, registry, calls, session } = makeContext();
    runCommand('pause', registry, ctx);
    expect(session.pauseToggle).toHaveBeenCalled();

    runCommand('frobnicate', registry, ctx);
    expect(calls.status).toHaveBeenCalledWith(expect.stringContaining('unknown command'));
  });

  it('cycles patterns with next/prev', () => {
    const { ctx, registry, calls } = makeContext();
    runCommand('next', registry, ctx); // calm -> equal
    expect(calls.setPattern).toHaveBeenLastCalledWith('equal');
    runCommand('prev', registry, ctx); // equal -> calm
    expect(calls.setPattern).toHaveBeenLastCalledWith('calm');
  });

  it('toggles the constellation appearance and accepts explicit modes', () => {
    const { ctx, registry, calls } = makeContext();
    runCommand('appearance', registry, ctx); // auto -> constellation
    expect(calls.setAppearance).toHaveBeenLastCalledWith('constellation');
    runCommand('appearance', registry, ctx); // constellation -> auto
    expect(calls.setAppearance).toHaveBeenLastCalledWith('auto');
    runCommand('appearance constellation', registry, ctx);
    expect(calls.setAppearance).toHaveBeenLastCalledWith('constellation');
    runCommand('sky auto', registry, ctx); // alias
    expect(calls.setAppearance).toHaveBeenLastCalledWith('auto');
  });

  it('rejects an unknown appearance arg without changing the setting', () => {
    const { ctx, registry, calls } = makeContext();
    runCommand('appearance dark', registry, ctx); // valid on the CLI, not the web
    expect(calls.setAppearance).not.toHaveBeenCalled();
    expect(calls.status).toHaveBeenCalledWith(
      expect.stringContaining('auto | constellation'),
    );
  });

  it('opens help as a page that lists visible commands', () => {
    const { ctx, registry, calls } = makeContext();
    runCommand('help', registry, ctx);
    expect(calls.page).toHaveBeenCalledTimes(1);
    expect(calls.page.mock.calls[0][0]).toContain('pause');
  });
});
