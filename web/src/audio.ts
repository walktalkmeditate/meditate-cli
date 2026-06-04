// The Web Audio engine: soundscapes, voices, and bells loaded live from the CDN
// (no download step), plus a zero-latency synth bell. URLs and behavior mirror
// the CLI (src/pack/mod.rs, src/audio/mod.rs); parity is asserted in audio.test.ts.
//
// Failures are designed, not thrown at the user: a CORS/offline/decode error
// becomes a calm one-line notice and falls back to the synth bell — the orb is
// never blocked.

const AUDIO_BASE = 'https://cdn.pilgrimapp.org/audio';
const VOICE_BASE = 'https://cdn.pilgrimapp.org/voiceguide';

// Mirrors src/audio/mod.rs.
const CROSSFADE_SECS = 4.0;
const DUCK_LEVEL = 0.35;
const DUCK_FADE_SECS = 0.5;

// Mirrors src/audio/bells.rs synth_bell: a struck C5 with inharmonic partials.
const BELL_BASE_HZ = 523.25;
const BELL_PARTIALS: ReadonlyArray<readonly [number, number]> = [
  [1.0, 1.0],
  [2.01, 0.5],
  [2.99, 0.25],
  [4.2, 0.12],
];
const BELL_SECS = 1.6;
const BELL_GAIN = 0.18;

export interface AudioAsset {
  id: string;
  type?: string;
  name?: string;
  displayName?: string;
}
interface AudioManifest {
  version?: string;
  assets?: AudioAsset[];
}
interface MeditationPrompt {
  id: string;
  seq?: number;
}
export interface VoicePack {
  id: string;
  name?: string;
  meditationPrompts?: MeditationPrompt[] | null;
}
interface VoiceManifest {
  version?: string;
  packs?: VoicePack[];
}

export type Notice = (message: string) => void;

/** The file URL for a soundscape or bell — `{base}/{type}/{id}.aac`. */
export function audioUrl(type: 'soundscape' | 'bell', id: string): string {
  return `${AUDIO_BASE}/${type}/${id}.aac`;
}

/** The file URL for a voice prompt — `{base}/{packId}/{promptId}.aac`. */
export function voiceUrl(packId: string, promptId: string): string {
  return `${VOICE_BASE}/${packId}/${promptId}.aac`;
}

/** Equal-power crossfade gains for progress `t` in [0,1] (sum of squares ≈ 1). */
export function equalPowerGains(t: number): { fadeIn: number; fadeOut: number } {
  const x = Math.min(1, Math.max(0, t));
  return { fadeIn: Math.sin((x * Math.PI) / 2), fadeOut: Math.cos((x * Math.PI) / 2) };
}

/** Soundscape/bell ids of a kind, filtered from the audio manifest by `type`. */
export function assetsOfType(manifest: AudioManifest, type: string): AudioAsset[] {
  return (manifest.assets ?? []).filter((a) => a.type === type);
}

/** Voice packs that carry meditation prompts (walk-only packs are ignored). */
export function meditationPacks(manifest: VoiceManifest): VoicePack[] {
  return (manifest.packs ?? []).filter((p) => (p.meditationPrompts ?? []).length > 0);
}

function equalPowerCurve(kind: 'in' | 'out'): Float32Array {
  const steps = 33;
  const curve = new Float32Array(steps);
  for (let i = 0; i < steps; i++) {
    const g = equalPowerGains(i / (steps - 1));
    curve[i] = kind === 'in' ? g.fadeIn : g.fadeOut;
  }
  return curve;
}

interface Soundscape {
  id: string;
  source: AudioBufferSourceNode;
  fade: GainNode; // crossfade envelope
  duck: GainNode; // voice ducking (separate, like the CLI's gain × duck)
}

export class AudioEngine {
  private ctx: AudioContext | null = null;
  private master: GainNode | null = null;
  private readonly buffers = new Map<string, AudioBuffer>();
  private audioManifest: AudioManifest | null = null;
  private voiceManifest: VoiceManifest | null = null;
  private soundscape: Soundscape | null = null;
  private voiceTimers: number[] = [];
  private voiceActive = false;
  private muteHinted = false;

  constructor(
    private readonly notice: Notice,
    private readonly touch = false,
  ) {}

  private context(): AudioContext {
    if (!this.ctx) {
      const Ctor = window.AudioContext ?? (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext;
      this.ctx = new Ctor();
      this.master = this.ctx.createGain();
      this.master.connect(this.ctx.destination);
    }
    return this.ctx;
  }

  /** Resume the context on a user gesture (iOS strictness) and warm it. */
  async unlock(): Promise<void> {
    const ctx = this.context();
    if (ctx.state !== 'running') {
      try {
        await ctx.resume();
      } catch {
        /* a denied resume just means no audio yet; the orb is unaffected */
      }
    }
  }

  // ── soundscapes ─────────────────────────────────────────────────────────────

  async listSoundscapes(): Promise<AudioAsset[]> {
    const m = await this.audioManifestOrNull();
    return m ? assetsOfType(m, 'soundscape') : [];
  }

  async playSoundscape(id: string): Promise<void> {
    const ctx = this.context();
    const buffer = await this.loadOrNotice(audioUrl('soundscape', id));
    if (!buffer) return;

    const fade = ctx.createGain();
    const duck = ctx.createGain();
    fade.connect(duck).connect(this.master!);
    const source = ctx.createBufferSource();
    source.buffer = buffer;
    source.loop = true;
    source.connect(fade);

    const now = ctx.currentTime;
    fade.gain.setValueAtTime(0, now);
    fade.gain.setValueCurveAtTime(equalPowerCurve('in'), now, CROSSFADE_SECS);
    // If a voice is mid-prompt, the new soundscape comes in already ducked.
    duck.gain.setValueAtTime(this.voiceActive ? DUCK_LEVEL : 1, now);
    source.start();

    this.fadeOutCurrent(now);
    this.soundscape = { id, source, fade, duck };
    this.maybeHintMute();
  }

  stopSoundscape(): void {
    if (!this.ctx) return;
    this.fadeOutCurrent(this.ctx.currentTime);
    this.soundscape = null;
  }

  private fadeOutCurrent(now: number): void {
    const prev = this.soundscape;
    if (!prev) return;
    prev.fade.gain.cancelScheduledValues(now);
    prev.fade.gain.setValueAtTime(prev.fade.gain.value, now);
    prev.fade.gain.setValueCurveAtTime(equalPowerCurve('out'), now, CROSSFADE_SECS);
    try {
      prev.source.stop(now + CROSSFADE_SECS + 0.1);
    } catch {
      /* already stopped */
    }
  }

  // ── voices ──────────────────────────────────────────────────────────────────

  async listVoices(): Promise<VoicePack[]> {
    const m = await this.voiceManifestOrNull();
    return m ? meditationPacks(m) : [];
  }

  /** Play a pack's meditation prompts in sequence, ducking the soundscape under
   *  each, looping the sequence. (The CLI schedules prompts against the breath;
   *  the web uses a timed sequence — a documented simplification.) */
  async playVoice(packId: string): Promise<void> {
    const m = await this.voiceManifestOrNull();
    const pack = m?.packs?.find((p) => p.id === packId);
    const prompts = (pack?.meditationPrompts ?? [])
      .slice()
      .sort((a, b) => (a.seq ?? 0) - (b.seq ?? 0));
    if (prompts.length === 0) {
      this.notice(`no meditation voice in '${packId}'`);
      return;
    }
    this.stopVoice();
    this.voiceActive = true;
    this.duckTo(DUCK_LEVEL);

    let index = 0;
    const playNext = async (): Promise<void> => {
      const prompt = prompts[index % prompts.length];
      index++;
      const buffer = await this.loadOrNotice(voiceUrl(packId, prompt.id));
      if (!buffer || !this.voiceActive) return;
      const ctx = this.context();
      const src = ctx.createBufferSource();
      src.buffer = buffer;
      src.connect(this.master!);
      src.start();
      const gapMs = (buffer.duration + 18) * 1000;
      this.voiceTimers.push(window.setTimeout(() => void playNext(), gapMs));
    };
    void playNext();
  }

  stopVoice(): void {
    this.voiceActive = false;
    this.voiceTimers.forEach((t) => window.clearTimeout(t));
    this.voiceTimers = [];
    this.duckTo(1);
  }

  private duckTo(level: number): void {
    if (!this.soundscape || !this.ctx) return;
    const g = this.soundscape.duck.gain;
    const now = this.ctx.currentTime;
    g.cancelScheduledValues(now);
    g.setValueAtTime(g.value, now);
    g.linearRampToValueAtTime(level, now + DUCK_FADE_SECS);
  }

  // ── bells ───────────────────────────────────────────────────────────────────

  /** Ring a downloaded bell, or the synth bell when none/unavailable. */
  async ring(bellId?: string): Promise<void> {
    if (bellId) {
      const buffer = await this.loadOrNotice(audioUrl('bell', bellId));
      if (buffer) {
        const ctx = this.context();
        const src = ctx.createBufferSource();
        src.buffer = buffer;
        src.connect(this.master!);
        src.start();
        return;
      }
      // fall through to the synth bell on failure
    }
    this.synthBell();
  }

  /** A struck C5 with inharmonic partials and an exponential decay — no network,
   *  no latency. Mirrors src/audio/bells.rs. */
  synthBell(): void {
    const ctx = this.context();
    const now = ctx.currentTime;
    const env = ctx.createGain();
    env.connect(this.master!);
    // exp(-3t) over BELL_SECS, scaled by BELL_GAIN.
    env.gain.setValueAtTime(BELL_GAIN, now);
    env.gain.exponentialRampToValueAtTime(BELL_GAIN * Math.exp(-3 * BELL_SECS), now + BELL_SECS);
    for (const [mult, amp] of BELL_PARTIALS) {
      const osc = ctx.createOscillator();
      osc.frequency.value = BELL_BASE_HZ * mult;
      const g = ctx.createGain();
      g.gain.value = amp;
      osc.connect(g).connect(env);
      osc.start(now);
      osc.stop(now + BELL_SECS);
    }
    this.maybeHintMute();
  }

  // ── loading + manifests ─────────────────────────────────────────────────────

  private async loadOrNotice(url: string): Promise<AudioBuffer | null> {
    const cached = this.buffers.get(url);
    if (cached) return cached;
    try {
      const res = await fetch(url, { mode: 'cors' });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data = await res.arrayBuffer();
      const buffer = await this.context().decodeAudioData(data);
      this.buffers.set(url, buffer);
      return buffer;
    } catch {
      this.notice('audio unavailable — breathing continues');
      return null;
    }
  }

  private async audioManifestOrNull(): Promise<AudioManifest | null> {
    if (this.audioManifest) return this.audioManifest;
    this.audioManifest = await this.fetchManifest<AudioManifest>(`${AUDIO_BASE}/manifest.json`);
    return this.audioManifest;
  }

  private async voiceManifestOrNull(): Promise<VoiceManifest | null> {
    if (this.voiceManifest) return this.voiceManifest;
    this.voiceManifest = await this.fetchManifest<VoiceManifest>(`${VOICE_BASE}/manifest.json`);
    return this.voiceManifest;
  }

  private async fetchManifest<T>(url: string): Promise<T | null> {
    try {
      const res = await fetch(url, { mode: 'cors' });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      return (await res.json()) as T;
    } catch {
      this.notice('sound packs unavailable right now');
      return null;
    }
  }

  /** One gentle hint on touch, where a silent ringer switch is the usual cause
   *  of "I hear nothing" (the API can't read the hardware mute directly). */
  private maybeHintMute(): void {
    if (this.touch && !this.muteHinted) {
      this.muteHinted = true;
      this.notice('(hear nothing? check your ringer / volume)');
    }
  }
}
