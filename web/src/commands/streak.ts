import { bold, dim, moss } from '../ansi';
import { todayOrdinal, type StreakStats } from '../store';
import { shareUrl } from '../deeplink';
import type { Command } from './types';

const WEEKS = 26;

/** Weekday (0=Sun..6=Sat) of a date ordinal — ordinals are UTC-based. */
function weekdayOf(ordinal: number): number {
  return new Date(ordinal * 86_400_000).getUTCDay();
}

/**
 * A GitHub-contribution-style grid: weeks as columns (oldest left), weekday rows
 * (Sun top → Sat bottom), the rightmost column the current week. Completed days
 * are moss squares; today is marked. Pure, so it unit-tests.
 */
export function renderHeatmap(completed: Set<number>, today: number, weeks = WEEKS): string {
  const sundayOfToday = today - weekdayOf(today);
  const labels = ['  ', 'M ', '  ', 'W ', '  ', 'F ', '  '];
  const rows: string[] = [];

  for (let r = 0; r < 7; r++) {
    let line = labels[r];
    for (let c = 0; c < weeks; c++) {
      const columnSunday = sundayOfToday - (weeks - 1 - c) * 7;
      const ord = columnSunday + r;
      const isToday = ord === today;
      if (ord > today) {
        line += ' ';
      } else if (completed.has(ord)) {
        line += isToday ? bold(moss('■')) : moss('■');
      } else {
        line += isToday ? dim('◦') : dim('·');
      }
    }
    rows.push(line);
  }
  return rows.join('\n');
}

export function streakPage(stats: StreakStats, grid: string): string {
  return [
    bold('your practice'),
    '',
    `  ${moss(`${stats.current}-day streak`)}${dim(
      `  ·  ${stats.rate30}/30 days  ·  ${stats.totalMinutes} min total  ·  longest ${stats.longest}`,
    )}`,
    '',
    grid,
    '',
    dim('  it lives only on this device — no account. `export` to keep a copy.'),
  ].join('\n');
}

export const streakCommand: Command = {
  name: 'streak',
  summary: 'your local practice + heatmap',
  run: (_args, ctx) => {
    const stats = ctx.store.stats();
    const grid = renderHeatmap(ctx.store.completedOrdinals(), todayOrdinal());
    ctx.page(streakPage(stats, grid));
  },
};

export const shareCommand: Command = {
  name: 'share',
  summary: 'copy a link to this session',
  run: async (_args, ctx) => {
    const url = shareUrl(ctx.shareLink());
    // On a phone, the native share sheet beats copying a URL into a terminal.
    // Called before any await, so the tap's transient activation still applies.
    if (navigator.share) {
      try {
        await navigator.share({ title: 'meditate', text: 'a breath in your terminal', url });
        return;
      } catch (err) {
        // The user dismissed the sheet — done; don't also copy behind their back.
        if (err instanceof DOMException && err.name === 'AbortError') return;
      }
    }
    try {
      await navigator.clipboard.writeText(url);
      ctx.status('link copied — share your breath');
    } catch {
      ctx.page([bold('share this session'), '', `  ${moss(url)}`].join('\n'));
    }
  },
};

export const exportCommand: Command = {
  name: 'export',
  summary: 'copy your local data',
  hidden: true,
  run: async (_args, ctx) => {
    const json = ctx.store.exportJson();
    try {
      await navigator.clipboard.writeText(json);
      ctx.status('your data — copied to clipboard');
    } catch {
      ctx.page([bold('your data'), '', json].join('\n'));
    }
  },
};
