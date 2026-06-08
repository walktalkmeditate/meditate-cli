// The Constellation backdrop: a Canvas-2D port of Pilgrim iOS's
// ConstellationOverlay (Pilgrim/Views/ConstellationOverlay.swift). It paints the
// flat indigo base, a cosmic gradient, drifting nebulae, twinkling parallax
// stars, and the occasional shooting star — behind the smooth orb, so the orb
// floats in the cosmos exactly as on iOS.
//
// Constants (colors, radii, opacities, drift speeds, twinkle range) are lifted
// from the iOS overlay so the two surfaces match. Positions are normalized 0..1
// so a resize reflows the field rather than reshuffling it.

const BASE_BG = '#0a0a12'; // flat indigo canvas, matching CanvasBackground.swift
const COSMIC_TINT: RGB = [26, 26, 41]; // 0.10,0.10,0.16 → brighter center
const STATIC_OPACITY = 0.6; // reduce-motion: no twinkle, mid brightness

const COOL_TINT: RGB = [232, 224, 255];
const WARM_TINT: RGB = [255, 232, 220];
const WARM_FRACTION = 0.3;

// Per depth layer: core radius (px), horizontal drift (px/s), vertical sway (px).
const LAYER_RADIUS = [1.2, 1.8, 2.6];
const LAYER_DRIFT = [0.4, 0.9, 1.6];
const LAYER_SWAY = [4, 6, 10];

const SHOOT_MIN_GAP = 30; // seconds
const SHOOT_MAX_GAP = 90;
const SHOOT_DURATION = 0.6; // seconds

type RGB = [number, number, number];

export interface Star {
  x: number; // normalized 0..1
  y: number;
  layer: number; // 0 far, 1 mid, 2 near
  radius: number;
  baseOpacity: number;
  twinkleHz: number;
  phase: number;
  tint: RGB;
}

export interface Nebula {
  x: number; // normalized 0..1
  y: number;
  radius: number; // px
  tint: RGB;
  driftSpeed: number; // px/s
}

interface ShootingLine {
  sx: number;
  sy: number;
  ex: number;
  ey: number;
}

const rand = (lo: number, hi: number): number => lo + Math.random() * (hi - lo);

/** 5–14 stars across three depth layers, mirroring iOS generateStars. */
export function generateStars(): Star[] {
  const count = Math.floor(rand(5, 15)); // 5..14
  return Array.from({ length: count }, () => {
    const layer = Math.floor(Math.random() * 3);
    return {
      x: rand(0.05, 0.95),
      y: rand(0.05, 0.95),
      layer,
      radius: LAYER_RADIUS[layer],
      baseOpacity: 0.6 + rand(0, 0.35),
      twinkleHz: rand(0.2, 0.4),
      phase: rand(0, Math.PI * 2),
      tint: Math.random() < WARM_FRACTION ? WARM_TINT : COOL_TINT,
    };
  });
}

/** 2–3 soft nebula clouds from the iOS candidate set. */
export function generateNebulae(): Nebula[] {
  const candidates: Nebula[] = [
    { x: 0.25, y: 0.2, radius: 280, tint: [158, 107, 235], driftSpeed: 0.6 }, // violet
    { x: 0.75, y: 0.55, radius: 340, tint: [102, 133, 235], driftSpeed: 0.4 }, // indigo
    { x: 0.45, y: 0.85, radius: 260, tint: [199, 133, 209], driftSpeed: 0.8 }, // plum
  ];
  for (let i = candidates.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [candidates[i], candidates[j]] = [candidates[j], candidates[i]];
  }
  return candidates.slice(0, Math.random() < 0.5 ? 2 : 3);
}

const rgba = (c: RGB, a: number): string => `rgba(${c[0]}, ${c[1]}, ${c[2]}, ${a})`;

export class Constellation {
  private readonly canvas: HTMLCanvasElement;
  private readonly ctx: CanvasRenderingContext2D;
  private w = 0;
  private h = 0;
  private stars: Star[];
  private nebulae: Nebula[];
  private readonly reduceMotion: boolean;
  private readonly reduceTransparency: boolean;
  private raf = 0;
  private clock = 0; // accumulated, dt-clamped seconds — drives all animation
  private lastT = -1; // last perf timestamp (s); -1 resets the dt clamp
  private cosmicGradient: CanvasGradient | null = null; // cached; rebuilt on resize
  private nextShootAt = 0; // seconds (clock)
  private shootStart = -1; // seconds, <0 = idle
  private shootLine: ShootingLine | null = null;

  constructor(parent: HTMLElement, includeNebulae = true) {
    this.reduceMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    this.reduceTransparency = window.matchMedia('(prefers-reduced-transparency: reduce)').matches;
    this.stars = generateStars();
    this.nebulae = includeNebulae ? generateNebulae() : [];

    this.canvas = document.createElement('canvas');
    this.canvas.id = 'constellation';
    this.canvas.style.position = 'absolute';
    this.canvas.style.inset = '0';
    this.canvas.style.pointerEvents = 'none';
    this.canvas.style.display = 'none';
    const ctx = this.canvas.getContext('2d');
    if (!ctx) throw new Error('no 2d context for the constellation');
    this.ctx = ctx;
    // Insert behind the terminal + smooth orb so the cosmos sits at the back.
    parent.insertBefore(this.canvas, parent.firstChild);
    this.resize();
  }

  resize(): void {
    const dpr = window.devicePixelRatio || 1;
    const rect = this.canvas.parentElement?.getBoundingClientRect();
    this.w = Math.max(1, rect?.width ?? window.innerWidth);
    this.h = Math.max(1, rect?.height ?? window.innerHeight);
    this.canvas.width = Math.round(this.w * dpr);
    this.canvas.height = Math.round(this.h * dpr);
    this.canvas.style.width = `${this.w}px`;
    this.canvas.style.height = `${this.h}px`;
    this.ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    this.cosmicGradient = null; // dimensions changed — rebuild on next paint
  }

  /** Run the backdrop's own rAF. `visible()` gates whether it shows and animates
   *  (constellation appearance selected and no page up). */
  start(visible: () => boolean): void {
    cancelAnimationFrame(this.raf); // idempotent: never leak a prior loop
    const frame = (): void => {
      this.raf = requestAnimationFrame(frame);
      if (document.hidden || !visible()) {
        if (this.canvas.style.display !== 'none') this.canvas.style.display = 'none';
        this.lastT = -1; // reset so the next visible frame doesn't jump the field
        return;
      }
      if (this.canvas.style.display !== 'block') this.canvas.style.display = 'block';
      // Accumulate a clamped clock so a tab returning from the background
      // resumes the field smoothly rather than jumping (mirrors SmoothOrb).
      const now = performance.now() / 1000;
      const dt = this.lastT < 0 ? 1 / 60 : Math.min(0.064, now - this.lastT);
      this.lastT = now;
      this.clock += dt;
      this.paint(this.clock);
    };
    this.raf = requestAnimationFrame(frame);
  }

  stop(): void {
    cancelAnimationFrame(this.raf);
  }

  private paint(t: number): void {
    const ctx = this.ctx;
    ctx.clearRect(0, 0, this.w, this.h);

    // Flat indigo base — always present, even under reduce-transparency.
    ctx.fillStyle = BASE_BG;
    ctx.fillRect(0, 0, this.w, this.h);

    // The overlay (gradient + nebulae + stars) is dropped under
    // reduce-transparency, leaving just the flat base — matching iOS.
    if (this.reduceTransparency) return;

    this.paintCosmicGradient();
    const animate = !this.reduceMotion;
    for (const n of this.nebulae) this.paintNebula(n, animate ? t : 0);
    for (const s of this.stars) this.paintStar(s, animate ? t : null);
    if (animate) this.paintShooting(t);
  }

  private paintCosmicGradient(): void {
    const ctx = this.ctx;
    // The gradient depends only on canvas size, so cache it and rebuild only
    // on resize rather than allocating a CanvasGradient every frame.
    if (!this.cosmicGradient) {
      const cx = this.w / 2;
      const cy = this.h / 2;
      const r = Math.max(this.w, this.h) * 0.7;
      const g = ctx.createRadialGradient(cx, cy, 0, cx, cy, r);
      g.addColorStop(0, rgba(COSMIC_TINT, 0.55));
      g.addColorStop(0.5, rgba(COSMIC_TINT, 0.18));
      g.addColorStop(1, rgba(COSMIC_TINT, 0));
      this.cosmicGradient = g;
    }
    ctx.fillStyle = this.cosmicGradient;
    ctx.fillRect(0, 0, this.w, this.h);
  }

  private paintNebula(n: Nebula, t: number): void {
    const ctx = this.ctx;
    const baseX = n.x * this.w;
    const cy = n.y * this.h;
    const cycle = this.w + n.radius * 2;
    let cx = (baseX + n.radius + t * n.driftSpeed) % cycle;
    if (cx < 0) cx += cycle;
    cx -= n.radius;

    const g = ctx.createRadialGradient(cx, cy, 0, cx, cy, n.radius);
    g.addColorStop(0, rgba(n.tint, 0.32));
    g.addColorStop(0.35, rgba(n.tint, 0.16));
    g.addColorStop(0.7, rgba(n.tint, 0.06));
    g.addColorStop(1, rgba(n.tint, 0));
    ctx.fillStyle = g;
    ctx.beginPath();
    ctx.arc(cx, cy, n.radius, 0, Math.PI * 2);
    ctx.fill();
  }

  private paintStar(s: Star, t: number | null): void {
    let x: number;
    let y: number;
    let opacity: number;

    if (t === null) {
      x = s.x * this.w;
      y = s.y * this.h;
      opacity = STATIC_OPACITY;
    } else {
      // Twinkle: baseOpacity * (0.5 + 0.5 sin).
      opacity = s.baseOpacity * (0.5 + 0.5 * Math.sin(t * 2 * Math.PI * s.twinkleHz + s.phase));
      // Parallax horizontal drift, wrapping past the edges.
      const cycle = this.w + 80;
      x = (s.x * this.w + t * LAYER_DRIFT[s.layer]) % cycle;
      if (x < 0) x += cycle;
      // Gentle vertical sway, deterministic per-star cadence.
      const swayPeriod = 30 + (s.phase / (2 * Math.PI)) * 30;
      const swayHz = 1 / swayPeriod;
      y = s.y * this.h + LAYER_SWAY[s.layer] * Math.sin(t * 2 * Math.PI * swayHz + s.phase);
    }

    const r = s.radius;
    // Halo → mid → core: three flat fills of decreasing size, increasing alpha.
    this.disc(x, y, r * 3.5, rgba(s.tint, opacity * 0.18));
    this.disc(x, y, r * 1.8, rgba(s.tint, opacity * 0.45));
    this.disc(x, y, r, rgba(s.tint, opacity));
  }

  private disc(x: number, y: number, radius: number, fill: string): void {
    const ctx = this.ctx;
    ctx.fillStyle = fill;
    ctx.beginPath();
    ctx.arc(x, y, radius, 0, Math.PI * 2);
    ctx.fill();
  }

  private paintShooting(t: number): void {
    if (this.nextShootAt === 0) {
      this.nextShootAt = t + rand(SHOOT_MIN_GAP, SHOOT_MAX_GAP);
    }
    if (this.shootStart < 0 && t >= this.nextShootAt) {
      this.shootStart = t;
      this.shootLine = this.randomShootingLine();
    }
    if (this.shootStart >= 0 && this.shootLine) {
      const elapsed = t - this.shootStart;
      if (elapsed >= SHOOT_DURATION) {
        this.shootStart = -1;
        this.shootLine = null;
        this.nextShootAt = t + rand(SHOOT_MIN_GAP, SHOOT_MAX_GAP);
        return;
      }
      const progress = elapsed / SHOOT_DURATION;
      const alpha = Math.sin(Math.PI * progress);
      const l = this.shootLine;
      const hx = l.sx + (l.ex - l.sx) * progress;
      const hy = l.sy + (l.ey - l.sy) * progress;
      const tp = Math.max(0, progress - 0.15);
      const tx = l.sx + (l.ex - l.sx) * tp;
      const ty = l.sy + (l.ey - l.sy) * tp;
      const ctx = this.ctx;
      ctx.strokeStyle = `rgba(255, 255, 255, ${alpha * 0.9})`;
      ctx.lineWidth = 1.5;
      ctx.lineCap = 'round';
      ctx.beginPath();
      ctx.moveTo(tx, ty);
      ctx.lineTo(hx, hy);
      ctx.stroke();
    }
  }

  private randomShootingLine(): ShootingLine {
    const fromLeft = Math.random() < 0.5;
    const sy = rand(0, this.h * 0.4);
    const sx = fromLeft ? rand(0, this.w * 0.3) : rand(this.w * 0.7, this.w);
    const length = this.w * rand(0.4, 0.6);
    const angle = rand(0.43, 0.79); // 25°–45°
    const dx = (fromLeft ? 1 : -1) * length * Math.cos(angle);
    const dy = length * Math.sin(angle);
    return { sx, sy, ex: sx + dx, ey: sy + dy };
  }
}
