import { describe, it, expect } from 'vitest';
import { relativeTime, renderBoot } from './boot';

const MIN = 60_000;
const HOUR = 60 * MIN;
const DAY = 24 * HOUR;

describe('relativeTime', () => {
  it('reads as a calm, friendly phrase across scales', () => {
    const now = 1_000_000_000;
    expect(relativeTime(now - 10_000, now)).toBe('just now');
    expect(relativeTime(now - 5 * MIN, now)).toBe('5 minutes ago');
    expect(relativeTime(now - HOUR, now)).toBe('1 hour ago');
    expect(relativeTime(now - 3 * HOUR, now)).toBe('3 hours ago');
    expect(relativeTime(now - DAY, now)).toBe('1 day ago');
    expect(relativeTime(now - 4 * DAY, now)).toBe('4 days ago');
  });

  it('never goes negative on a clock skew', () => {
    expect(relativeTime(1000, 0)).toBe('just now');
  });
});

describe('renderBoot', () => {
  it('shows a Last login line for a returning visitor', () => {
    const out = renderBoot('0.2.1', 1000, 1000 + 2 * HOUR);
    expect(out).toContain('Last login: 2 hours ago on cli.pilgrimapp.org');
    expect(out).toContain('meditate');
  });

  it('welcomes a first-time visitor', () => {
    const out = renderBoot('0.2.1', null, Date.now());
    expect(out).toContain('first breath');
    expect(out).not.toContain('Last login');
  });
});
