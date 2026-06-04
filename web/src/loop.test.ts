import { describe, it, expect } from 'vitest';
import { frameSequence, shouldDraw } from './loop';

describe('frameSequence', () => {
  it('wraps content in synchronized output and homes the cursor', () => {
    const seq = frameSequence('ORB');
    expect(seq.startsWith('\x1b[?2026h')).toBe(true);
    expect(seq.endsWith('\x1b[?2026l')).toBe(true);
    expect(seq).toContain('\x1b[H');
    expect(seq).toContain('ORB');
  });

  it('never clears the screen (no flash)', () => {
    expect(frameSequence('anything')).not.toContain('\x1b[2J');
  });
});

describe('shouldDraw', () => {
  it('always draws the first frame', () => {
    expect(shouldDraw(-1, 0, 33)).toBe(true);
  });

  it('throttles to the frame interval', () => {
    expect(shouldDraw(100, 110, 33)).toBe(false); // 10ms < 33ms
    expect(shouldDraw(100, 140, 33)).toBe(true); // 40ms >= 33ms
  });

  it('caps an unthrottled loop at ~30fps with a 33ms interval', () => {
    let last = -1;
    let drawn = 0;
    for (let t = 0; t <= 1000; t += 4) {
      if (shouldDraw(last, t, 1000 / 30)) {
        drawn++;
        last = t;
      }
    }
    expect(drawn).toBeLessThanOrEqual(31);
  });
});
