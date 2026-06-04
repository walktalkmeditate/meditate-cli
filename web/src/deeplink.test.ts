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

  it('composes a full share URL from a base', () => {
    expect(shareUrl({ pattern: 'box' }, 'https://cli.pilgrimapp.org/')).toBe(
      'https://cli.pilgrimapp.org/#p=box',
    );
  });
});
