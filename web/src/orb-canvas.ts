// The smooth orb: a Canvas-2D overlay matching the Pilgrim iOS meditation orb,
// driven entirely by the WASM breath accessors (scale, glow, palette, breath
// count) so it stays phase-locked with the terminal orb — one clock, no drift.
//
// The gradient stops, radii, scale factors, and layer structure are lifted
// verbatim from ../pilgrim-ios/Pilgrim/Scenes/ActiveWalk/MeditationView.swift
// (the orb body, lines 235-275): a soft outer halo, a bright inner core, and
// moss ripple rings. The background stays terminal-dark (we do not adopt iOS's
// parchment), so the moss glows. We deliberately omit the drifting fog/stars —
// in Pilgrim those belong to the separate Constellation appearance mode, not the
// orb, so here the background stays clean.

import type { Session } from './wasm/meditate_wasm.js';

// iOS frame sizes are in points; we scale them by `unit` so the orb's footprint
// matches the half-block orb (radius ≈ 0.46·min(w,h) at full breath).
const HALO_FRAME = 320; // pt — outer halo
const CORE_FRAME = 160; // pt — inner core
const HALO_START = 20;
const HALO_END = 160;
const CORE_END = 80;
const FOOTPRINT = 0.46;

const RIPPLE_MS = 3000;

// Voice rings: while a guide speaks, four outer rings vibrate around the orb
// (mirrors iOS MeditationView voiceRingLayer). Radii are multiples of the halo
// radius; the pulse oscillates their scale + opacity over ~2.5s.
// Each voice ring as one record, so factor/opacity/irregularity can't drift out
// of sync — a parallel-array length mismatch would silently paint a NaN ring.
const VOICE_RINGS = [
  { factor: 1.06, opacity: 0.24, irreg: 0.6 },
  { factor: 1.2, opacity: 0.18, irreg: -0.9 },
  { factor: 1.34, opacity: 0.13, irreg: 0.4 },
  { factor: 1.48, opacity: 0.09, irreg: -0.5 },
];
const VOICE_PULSE_MS = 2500;

interface Ripple {
  life: number; // 0..1
}

export class SmoothOrb {
  private readonly canvas: HTMLCanvasElement;
  private readonly ctx: CanvasRenderingContext2D;
  private w = 0;
  private h = 0;
  private ripples: Ripple[] = [];
  private lastBreath = -1;
  private fade = 0; // cross-fade 0..1 between block and smooth
  private voiceEnv = 0; // 0..1, eased up while a voice prompt speaks
  private clock = 0; // ms, drives the voice-ring pulse
  private lastT = -1;
  private raf = 0;

  constructor(parent: HTMLElement) {
    this.canvas = document.createElement('canvas');
    this.canvas.id = 'orb';
    const ctx = this.canvas.getContext('2d');
    if (!ctx) throw new Error('no 2d context for the smooth orb');
    this.ctx = ctx;
    parent.appendChild(this.canvas);
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
  start(session: Session, visible: () => boolean, voiceActive: () => boolean): void {
    const frame = (t: number): void => {
      this.raf = requestAnimationFrame(frame);
      if (document.hidden) return; // no canvas work for a hidden tab
      const dt = this.lastT < 0 ? 16 : Math.min(64, t - this.lastT);
      this.lastT = t;
      this.clock += dt;

      // Ease a 0..1 envelope toward 1 while a prompt speaks (drives the rings).
      this.voiceEnv += ((voiceActive() ? 1 : 0) - this.voiceEnv) * Math.min(1, dt / 500);

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

  private advance(session: Session, dt: number): void {
    const breath = session.breathCount();
    if (this.lastBreath >= 0 && breath > this.lastBreath) {
      this.ripples.push({ life: 0 });
      if (this.ripples.length > 3) this.ripples.shift();
    }
    this.lastBreath = breath;

    for (const r of this.ripples) r.life += dt / RIPPLE_MS;
    this.ripples = this.ripples.filter((r) => r.life < 1);
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

    // Moss ripple rings — emitted on each completed breath, expanding + fading.
    for (const r of this.ripples) {
      const radius = (40 + 160 * r.life) * unit;
      ctx.strokeStyle = `rgba(${pal[0]}, ${pal[1]}, ${pal[2]}, ${0.5 * (1 - r.life)})`;
      ctx.lineWidth = 0.6 * unit;
      ctx.beginPath();
      ctx.arc(cx, cy, radius, 0, Math.PI * 2);
      ctx.stroke();
    }

    // Voice rings — while a guide speaks, four outer rings vibrate (scale +
    // opacity pulse) and the core softens. Mirrors the iOS voiceRingLayer.
    const pulse = 0.5 + 0.5 * Math.sin((this.clock / VOICE_PULSE_MS) * Math.PI * 2);
    const soften = 1 - this.voiceEnv * 0.16;
    if (this.voiceEnv > 0.01) {
      const ringScale = 0.97 + 0.07 * pulse;
      const ringOpacity = 0.6 + 0.4 * pulse;
      const baseR = HALO_END * unit;
      for (const ring of VOICE_RINGS) {
        const r = baseR * (ring.factor + ring.irreg * 0.02) * ringScale;
        ctx.strokeStyle = moss(ring.opacity * this.voiceEnv * ringOpacity);
        ctx.lineWidth = 1.2;
        ctx.beginPath();
        ctx.arc(cx, cy, r, 0, Math.PI * 2);
        ctx.stroke();
      }
    }

    // Inner core — moss 0.7+glow·0.2 → 0.3+glow·0.1, startRadius 0, endRadius 80.
    const core = ctx.createRadialGradient(cx, cy, 0, cx, cy, CORE_END * s);
    core.addColorStop(0, moss((0.7 + glow * 0.2) * soften));
    core.addColorStop(1, moss((0.3 + glow * 0.1) * soften));
    ctx.fillStyle = core;
    ctx.beginPath();
    ctx.arc(cx, cy, (CORE_FRAME / 2) * s, 0, Math.PI * 2);
    ctx.fill();

    // (No steady outline — the one intentional ring is the per-breath ripple
    // pulse above, so nothing flickers in and out with the breath.)

    ctx.globalAlpha = 1;
  }
}
