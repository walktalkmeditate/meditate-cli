// The smooth orb: a Canvas-2D overlay matching the Pilgrim iOS meditation orb,
// driven entirely by the WASM breath accessors (scale, glow, palette, breath
// count) so it stays phase-locked with the terminal orb — one clock, no drift.
//
// The gradient stops, radii, scale factors, and layer structure are lifted
// verbatim from ../pilgrim-ios/Pilgrim/Scenes/ActiveWalk/MeditationView.swift
// (the orb body, lines 235-275): a soft outer halo, a bright inner core, and a
// thin ring, with moss ripple rings and drifting fog particles. The background
// stays terminal-dark (we do not adopt iOS's parchment), so the moss glows.

import type { Session } from './wasm/meditate_wasm.js';

// iOS frame sizes are in points; we scale them by `unit` so the orb's footprint
// matches the half-block orb (radius ≈ 0.46·min(w,h) at full breath).
const HALO_FRAME = 320; // pt — outer halo
const CORE_FRAME = 160; // pt — inner core
const HALO_START = 20;
const HALO_END = 160;
const CORE_END = 80;
const FOOTPRINT = 0.46;

const FOG_RGB = '216, 224, 214'; // fog particle color (light, over the dark orb)

const PARTICLE_COUNT = 14;
const PARTICLE_GLOW_MS = 6000; // iOS: 6s ease-in-out glow cycle
const RIPPLE_MS = 3000;

interface Particle {
  x: number; // 0..1 of width
  y: number; // 0..1 of height
  size: number; // px
  base: number; // base opacity
  drift: number; // vertical drift px/s
  phase: number; // glow phase offset
}

interface Ripple {
  life: number; // 0..1
}

function rand(min: number, max: number): number {
  return min + Math.random() * (max - min);
}

export class SmoothOrb {
  private readonly canvas: HTMLCanvasElement;
  private readonly ctx: CanvasRenderingContext2D;
  private w = 0;
  private h = 0;
  private particles: Particle[] = [];
  private ripples: Ripple[] = [];
  private lastBreath = -1;
  private fade = 0; // cross-fade 0..1 between block and smooth
  private clock = 0;
  private lastT = -1;
  private raf = 0;

  constructor(parent: HTMLElement) {
    this.canvas = document.createElement('canvas');
    this.canvas.id = 'orb';
    const ctx = this.canvas.getContext('2d');
    if (!ctx) throw new Error('no 2d context for the smooth orb');
    this.ctx = ctx;
    parent.appendChild(this.canvas);
    this.seedParticles();
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
  }

  /** Run the orb's own rAF (independent of the breath loop, so it keeps fading
   *  cleanly even while a help page is up). `visible()` gates block vs smooth. */
  start(session: Session, visible: () => boolean): void {
    const frame = (t: number): void => {
      this.raf = requestAnimationFrame(frame);
      if (document.hidden) return; // no canvas work for a hidden tab
      const dt = this.lastT < 0 ? 16 : Math.min(64, t - this.lastT);
      this.lastT = t;
      this.clock += dt;

      const target = visible() ? 1 : 0;
      this.fade += (target - this.fade) * Math.min(1, dt / 200);
      if (this.fade < 0.01 && target === 0) {
        this.ctx.clearRect(0, 0, this.w, this.h);
        return;
      }
      this.advance(session, dt);
      this.paint(session);
    };
    this.raf = requestAnimationFrame(frame);
  }

  stop(): void {
    cancelAnimationFrame(this.raf);
  }

  private seedParticles(): void {
    this.particles = Array.from({ length: PARTICLE_COUNT }, () => ({
      x: rand(0.15, 0.85),
      y: rand(0.15, 0.85),
      size: rand(1.5, 4),
      base: rand(0.08, 0.3),
      drift: rand(-6, -1),
      phase: rand(0, PARTICLE_GLOW_MS),
    }));
  }

  private advance(session: Session, dt: number): void {
    const breath = session.breathCount();
    if (this.lastBreath >= 0 && breath > this.lastBreath) {
      this.ripples.push({ life: 0 });
      if (this.ripples.length > 3) this.ripples.shift();
    }
    this.lastBreath = breath;

    for (const r of this.ripples) r.life += dt / RIPPLE_MS;
    this.ripples = this.ripples.filter((r) => r.life < 1);

    for (const p of this.particles) {
      p.y += (p.drift * dt) / 1000 / this.h;
      if (p.y < 0.1) p.y = 0.9;
    }
  }

  private paint(session: Session): void {
    const ctx = this.ctx;
    ctx.clearRect(0, 0, this.w, this.h);
    ctx.globalAlpha = this.fade;

    const scale = session.scale();
    const glow = session.glow();
    const pal = session.palette();
    if (pal.length < 9) return; // core/edge/ripple RGB; bail rather than paint NaN colors
    const moss = (a: number): string => `rgba(${pal[0]}, ${pal[1]}, ${pal[2]}, ${a})`;

    const cx = this.w / 2;
    const cy = this.h / 2;
    const unit = (Math.min(this.w, this.h) * FOOTPRINT) / HALO_END;
    const s = unit * scale;

    // Outer halo — moss 0.5 → 0.15 → 0.0, startRadius 20, endRadius 160 (×320pt).
    const halo = ctx.createRadialGradient(cx, cy, HALO_START * s, cx, cy, HALO_END * s);
    halo.addColorStop(0, moss(0.5));
    halo.addColorStop(0.5, moss(0.15));
    halo.addColorStop(1, moss(0));
    ctx.fillStyle = halo;
    ctx.beginPath();
    ctx.arc(cx, cy, (HALO_FRAME / 2) * s, 0, Math.PI * 2);
    ctx.fill();

    // Drifting fog particles (under the core, like the iOS particle layer).
    this.paintParticles(cx, cy, unit);

    // Moss ripple rings — emitted on each completed breath, expanding + fading.
    for (const r of this.ripples) {
      const radius = (40 + 160 * r.life) * unit;
      ctx.strokeStyle = `rgba(${pal[0]}, ${pal[1]}, ${pal[2]}, ${0.5 * (1 - r.life)})`;
      ctx.lineWidth = 0.6 * unit;
      ctx.beginPath();
      ctx.arc(cx, cy, radius, 0, Math.PI * 2);
      ctx.stroke();
    }

    // Inner core — moss 0.7+glow·0.2 → 0.3+glow·0.1, startRadius 0, endRadius 80.
    const core = ctx.createRadialGradient(cx, cy, 0, cx, cy, CORE_END * s);
    core.addColorStop(0, moss(0.7 + glow * 0.2));
    core.addColorStop(1, moss(0.3 + glow * 0.1));
    ctx.fillStyle = core;
    ctx.beginPath();
    ctx.arc(cx, cy, (CORE_FRAME / 2) * s, 0, Math.PI * 2);
    ctx.fill();

    // (No steady outline — the one intentional ring is the per-breath ripple
    // pulse above, so nothing flickers in and out with the breath.)

    ctx.globalAlpha = 1;
  }

  private paintParticles(cx: number, cy: number, unit: number): void {
    const ctx = this.ctx;
    const spread = HALO_END * unit * 1.1;
    for (const p of this.particles) {
      // iOS oscillates particle glow over a 6s ease-in-out cycle.
      const t = ((this.clock + p.phase) % PARTICLE_GLOW_MS) / PARTICLE_GLOW_MS;
      const glow = 0.5 + 0.5 * Math.sin(t * Math.PI * 2);
      const alpha = p.base * (0.5 + glow);
      const px = cx + (p.x - 0.5) * 2 * spread;
      const py = cy + (p.y - 0.5) * 2 * spread;
      const g = ctx.createRadialGradient(px, py, 0, px, py, p.size * 2);
      g.addColorStop(0, `rgba(${FOG_RGB}, ${alpha})`);
      g.addColorStop(1, `rgba(${FOG_RGB}, 0)`);
      ctx.fillStyle = g;
      ctx.beginPath();
      ctx.arc(px, py, p.size * 2, 0, Math.PI * 2);
      ctx.fill();
    }
  }
}
