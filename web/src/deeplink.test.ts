import { describe, it, expect } from 'vitest';
import { parseHash, buildHash, shareUrl } from './deeplink';

describe('deep-link hash', () => {
  it('parses a known pattern and sound', () => {
    expect(parseHash('#p=box&snd=rain')).toEqual({ pattern: 'box', sound: 'rain' });
  });

  it('flags an unknown pattern for fallback instead of accepting it', () => {
    const link = parseHash('#p=sideways');
    expect(link.pattern).toBeUndefined();
    expect(link.invalidPattern).toBe('sideways');
  });

  it('round-trips through buildHash', () => {
    const link = { pattern: 'calm', sound: 'forest' };
    expect(parseHash(buildHash(link))).toEqual(link);
  });

  it('builds an empty hash for an empty link', () => {
    expect(buildHash({})).toBe('');
  });

  it('rejects a sound id that is not in the asset alphabet (escape-injection guard)', () => {
    // #given a hash whose sound carries a percent-encoded ESC + OSC introducer
    const hash = '#snd=%1b%5d8';
    // #when parsed
    const link = parseHash(hash);
    // #then the unsafe value is dropped entirely, never reaching the terminal
    expect(link.sound).toBeUndefined();
  });

  it('accepts a normal sound id and rejects an over-long one', () => {
    expect(parseHash('#snd=forest-rain').sound).toBe('forest-rain');
    expect(parseHash(`#snd=${'a'.repeat(64)}`).sound).toBeUndefined();
  });

  it('neutralizes control bytes in an invalid pattern before it is echoed', () => {
    // #given an invalid pattern value containing a raw control byte
    const link = parseHash('#p=%1bX');
    // #then the echoed fallback string carries no ESC/control bytes
    expect(link.invalidPattern).toBeDefined();
    // eslint-disable-next-line no-control-regex
    expect(/[\x00-\x1f\x7f-\x9f]/.test(link.invalidPattern ?? '')).toBe(false);
  });

  it('composes a full share URL from a base', () => {
    expect(shareUrl({ pattern: 'box' }, 'https://cli.pilgrimapp.org/')).toBe(
      'https://cli.pilgrimapp.org/#p=box',
    );
  });
});
