import { describe, it, expect, vi, beforeEach } from 'vitest';
import { deriveStats, ordinalFromKey, localDayKey, Persistence } from './store';

const ord = (key: string): number => ordinalFromKey(key);

describe('streak math (parity with src/streak.rs)', () => {
  // Anchor "today" to a fixed date so the tests are deterministic.
  const today = ord('2026-06-03');

  it('counts a consecutive run including today', () => {
    const days = new Set(['2026-06-01', '2026-06-02', '2026-06-03'].map(ord));
    expect(deriveStats(days, today, 0).current).toBe(3);
  });

  it('a gap resets the current streak but the 30-day rate persists', () => {
    // Three days a week ago, then today — the current run is just today (1),
    // but four days fall within the last 30 (the rate persists).
    const days = new Set(
      ['2026-05-25', '2026-05-26', '2026-05-27', '2026-06-03'].map(ord),
    );
    const stats = deriveStats(days, today, 0);
    expect(stats.current).toBe(1);
    expect(stats.longest).toBe(3);
    expect(stats.rate30).toBe(4);
  });

  it('keeps the current run alive when today is not yet done but yesterday was', () => {
    const days = new Set(['2026-06-01', '2026-06-02'].map(ord));
    expect(deriveStats(days, today, 0).current).toBe(2); // anchored at yesterday
  });

  it('breaks the run when neither today nor yesterday is done', () => {
    const days = new Set(['2026-05-30', '2026-05-31'].map(ord));
    expect(deriveStats(days, today, 0).current).toBe(0);
  });

  it('reports total minutes from accrued seconds', () => {
    expect(deriveStats(new Set(), today, 605).totalMinutes).toBe(10);
  });
});

describe('local day keys', () => {
  it('formats a zero-padded YYYY-MM-DD', () => {
    expect(localDayKey(new Date(2026, 0, 5))).toBe('2026-01-05');
  });

  it('ordinals are consecutive across a month boundary', () => {
    expect(ordinalFromKey('2026-02-01') - ordinalFromKey('2026-01-31')).toBe(1);
  });
});

describe('Persistence import/export', () => {
  beforeEach(() => {
    const mem = new Map<string, string>();
    vi.stubGlobal('localStorage', {
      getItem: (k: string) => mem.get(k) ?? null,
      setItem: (k: string, v: string) => mem.set(k, v),
      removeItem: (k: string) => mem.delete(k),
    });
  });

  it('round-trips lastVisit through export then import', () => {
    // #given a store that recorded a visit
    const a = new Persistence();
    a.markVisit(1234);
    const json = a.exportJson();
    // #when imported into a fresh store
    const b = new Persistence();
    // #then the import succeeds and lastVisit survives (not reset to "first breath")
    expect(b.importJson(json)).toBe(true);
    expect(b.lastVisit()).toBe(1234);
  });

  it('returns false on malformed JSON', () => {
    expect(new Persistence().importJson('{ not json')).toBe(false);
  });
});
