// A favicon that breathes: a small canvas orb whose radius and glow track the
// breath, pushed to the browser tab icon a few times a second. It reads the
// same breath-state accessors the terminal orb does, so the tab icon pulses in
// lockstep with the orb — a tiny preview of the U6 Canvas-2D orb.

const SIZE = 64;
// Redraw only when the orb has visibly changed, so a held breath (constant
// scale) costs nothing and an inhale updates ~7×/s — smooth and cheap.
const SCALE_EPSILON = 0.015;

function rgb(p: Uint8Array, i: number): string {
  return `rgb(${p[i]}, ${p[i + 1]}, ${p[i + 2]})`;
}

/** Mix a color toward white by `t` (0..1), for the held-breath inner glow. */
function lighten(p: Uint8Array, i: number, t: number): string {
  const m = (v: number) => Math.round(v + (255 - v) * t);
  return `rgb(${m(p[i])}, ${m(p[i + 1])}, ${m(p[i + 2])})`;
}

export class BreathingFavicon {
  private readonly canvas: HTMLCanvasElement;
  private readonly ctx: CanvasRenderingContext2D;
  private lastScale = -1;

  constructor() {
    this.canvas = document.createElement('canvas');
    this.canvas.width = SIZE;
    this.canvas.height = SIZE;
    const ctx = this.canvas.getContext('2d');
    if (!ctx) throw new Error('no 2d context for the favicon');
    this.ctx = ctx;
  }

  /** Update the tab icon for the current breath. No-op until it visibly moves. */
  update(scale: number, glow: number, palette: Uint8Array): void {
    if (palette.length < 9) return;
    if (Math.abs(scale - this.lastScale) < SCALE_EPSILON) return;
    this.lastScale = scale;
    this.draw(scale, glow, palette);
    this.apply(this.canvas.toDataURL('image/png'));
  }

  private draw(scale: number, glow: number, palette: Uint8Array): void {
    const ctx = this.ctx;
    const s = SIZE;
    const cx = s / 2;
    const cy = s / 2;
    const radius = Math.max(2, s * 0.42 * scale);

    ctx.clearRect(0, 0, s, s);
    ctx.fillStyle = '#0a0c10';
    ctx.beginPath();
    ctx.roundRect(0, 0, s, s, s * 0.22);
    ctx.fill();

    const gradient = ctx.createRadialGradient(
      cx,
      cy * 0.9,
      radius * 0.1,
      cx,
      cy,
      radius,
    );
    gradient.addColorStop(0, lighten(palette, 0, glow * 0.45));
    gradient.addColorStop(0.6, rgb(palette, 0));
    gradient.addColorStop(1, rgb(palette, 3));

    ctx.fillStyle = gradient;
    ctx.beginPath();
    ctx.arc(cx, cy, radius, 0, Math.PI * 2);
    ctx.fill();
  }

  /** Swap in a fresh <link rel="icon"> — replacing the node forces the tab to
   *  refresh the icon reliably across browsers. */
  private apply(href: string): void {
    document
      .querySelectorAll('link[rel="icon"]')
      .forEach((el) => el.remove());
    const link = document.createElement('link');
    link.rel = 'icon';
    link.type = 'image/png';
    link.href = href;
    document.head.appendChild(link);
  }
}
