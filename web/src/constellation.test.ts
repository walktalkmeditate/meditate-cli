import { describe, it, expect } from 'vitest';
import { generateStars, generateNebulae } from './constellation';

// Parity with iOS ConstellationStarGenerationTests — same ranges and invariants.
describe('constellation generation', () => {
  it('generates 5–14 stars with normalized positions and slow twinkle', () => {
    for (let i = 0; i < 50; i++) {
      const stars = generateStars();
      expect(stars.length).toBeGreaterThanOrEqual(5);
      expect(stars.length).toBeLessThanOrEqual(14);
      for (const s of stars) {
        expect(s.x).toBeGreaterThanOrEqual(0);
        expect(s.x).toBeLessThanOrEqual(1);
        expect(s.y).toBeGreaterThanOrEqual(0);
        expect(s.y).toBeLessThanOrEqual(1);
        // WCAG 2.3.1: twinkle stays slow (iOS target ≤ 0.5 Hz).
        expect(s.twinkleHz).toBeGreaterThan(0);
        expect(s.twinkleHz).toBeLessThanOrEqual(0.5);
        expect(s.layer).toBeGreaterThanOrEqual(0);
        expect(s.layer).toBeLessThanOrEqual(2);
      }
    }
  });

  it('generates 2–3 nebulae with normalized positions', () => {
    for (let i = 0; i < 50; i++) {
      const nebulae = generateNebulae();
      expect(nebulae.length).toBeGreaterThanOrEqual(2);
      expect(nebulae.length).toBeLessThanOrEqual(3);
      for (const n of nebulae) {
        expect(n.x).toBeGreaterThanOrEqual(0);
        expect(n.x).toBeLessThanOrEqual(1);
        expect(n.y).toBeGreaterThanOrEqual(0);
        expect(n.y).toBeLessThanOrEqual(1);
        expect(n.radius).toBeGreaterThan(0);
      }
    }
  });
});
