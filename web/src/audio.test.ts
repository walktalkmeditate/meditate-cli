import { describe, it, expect } from 'vitest';
import {
  audioUrl,
  voiceUrl,
  equalPowerGains,
  assetsOfType,
  meditationPacks,
} from './audio';

describe('CDN URL parity with the CLI (src/pack/mod.rs)', () => {
  it('builds soundscape and bell URLs as {base}/{type}/{id}.aac', () => {
    expect(audioUrl('soundscape', 'rain')).toBe(
      'https://cdn.pilgrimapp.org/audio/soundscape/rain.aac',
    );
    expect(audioUrl('bell', 'tibetan')).toBe(
      'https://cdn.pilgrimapp.org/audio/bell/tibetan.aac',
    );
  });

  it('builds voice URLs as {base}/{packId}/{promptId}.aac', () => {
    expect(voiceUrl('calm-pack', 'intro')).toBe(
      'https://cdn.pilgrimapp.org/voiceguide/calm-pack/intro.aac',
    );
  });
});

describe('manifest filtering', () => {
  it('filters audio assets by type (soundscapes vs bells)', () => {
    const manifest = {
      assets: [
        { id: 'rain', type: 'soundscape' },
        { id: 'bell1', type: 'bell' },
        { id: 'forest', type: 'soundscape' },
      ],
    };
    expect(assetsOfType(manifest, 'soundscape').map((a) => a.id)).toEqual(['rain', 'forest']);
    expect(assetsOfType(manifest, 'bell').map((a) => a.id)).toEqual(['bell1']);
  });

  it('keeps only voice packs that carry meditation prompts (ignores walk-only)', () => {
    const manifest = {
      packs: [
        { id: 'med', meditationPrompts: [{ id: 'a' }] },
        { id: 'walk-only', meditationPrompts: [] },
        { id: 'null-prompts', meditationPrompts: null },
      ],
    };
    expect(meditationPacks(manifest).map((p) => p.id)).toEqual(['med']);
  });
});

describe('equal-power crossfade', () => {
  it('preserves roughly constant power across the fade', () => {
    for (const t of [0, 0.25, 0.5, 0.75, 1]) {
      const { fadeIn, fadeOut } = equalPowerGains(t);
      expect(fadeIn ** 2 + fadeOut ** 2).toBeCloseTo(1, 5);
    }
  });

  it('starts silent-to-full and clamps out of range', () => {
    expect(equalPowerGains(0).fadeIn).toBeCloseTo(0, 5);
    expect(equalPowerGains(1).fadeIn).toBeCloseTo(1, 5);
    expect(equalPowerGains(-1).fadeIn).toBeCloseTo(0, 5); // clamped
    expect(equalPowerGains(2).fadeOut).toBeCloseTo(0, 5); // clamped
  });
});
