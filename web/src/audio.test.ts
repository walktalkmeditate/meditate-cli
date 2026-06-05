import { describe, it, expect } from 'vitest';
import {
  audioUrl,
  voiceUrl,
  equalPowerGains,
  assetsOfType,
  meditationPacks,
  crossfadeLoopChannel,
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

describe('seamless loop crossfade', () => {
  it('shortens the channel by the crossfade length and makes the wrap continuous', () => {
    // #given a ramp 0..9 where the raw wrap (9 -> 0) is a hard discontinuity
    const src = Float32Array.from({ length: 10 }, (_, i) => i);

    // #when we fold a 2-sample tail into the head
    const out = crossfadeLoopChannel(src, 2);

    // #then the loop is `len - xfade` long, the body past the fade is untouched,
    // and out[0] picks up the tail (src[len - xfade]) so out[last] -> out[0]
    // continues the original sequence (src[7] -> src[8]) instead of (9 -> 0).
    expect(out.length).toBe(8);
    expect(out[2]).toBeCloseTo(2, 5); // body unchanged from index xfade on
    expect(out[7]).toBeCloseTo(7, 5); // last sample is src[7]
    expect(out[0]).toBeCloseTo(8, 5); // = src[len - xfade] = src[8], adjacent to 7
  });

  it('returns a copy when the clip is too short to fold', () => {
    const src = Float32Array.from([5]);
    const out = crossfadeLoopChannel(src, 5);
    expect(Array.from(out)).toEqual([5]);
    expect(out).not.toBe(src); // a copy, not the same reference
  });
});
