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
// Tail-into-head crossfade length for a seamless soundscape loop (no audible
// restart at the wrap). Mirrors src/audio/mod.rs.
const LOOP_CROSSFADE_SECS = 3.0;
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

// Silence between meditation voice prompts — a bounded random gap, so the guide
// arrives with a gentle element of surprise rather than a fixed cadence (never
// rushed, never abandoned). The CLI applies the same jitter idea over a longer
// range (MIN/MAX_SPACING_SECS in src/audio/voice.rs).
const VOICE_GAP_MIN_SECS = 45;
const VOICE_GAP_MAX_SECS = 105;

function voiceGapSecs(): number {
  return VOICE_GAP_MIN_SECS + Math.random() * (VOICE_GAP_MAX_SECS - VOICE_GAP_MIN_SECS);
}

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

/** Crossfade a channel's tail into its head so it loops seamlessly. Returns a
 *  buffer of length `len − xfade`: each sample in the first `xfade` blends the
 *  head (fading in) with the folded-in tail (fading out), so the wrap from the
 *  last sample back to the first is continuous. Pure, so audio.test.ts can cover
 *  it without an AudioContext. */
export function crossfadeLoopChannel(src: Float32Array, xfade: number): Float32Array {
  const x = Math.min(Math.max(0, Math.floor(xfade)), Math.floor(src.length / 2));
  if (x < 1) return src.slice();
  const outLen = src.length - x;
  const out = new Float32Array(outLen);
  for (let i = 0; i < outLen; i++) {
    if (i < x) {
      const { fadeIn, fadeOut } = equalPowerGains(i / x);
      out[i] = src[i] * fadeIn + src[i + outLen] * fadeOut;
    } else {
      out[i] = src[i];
    }
  }
  return out;
}

/** A seamless-loop copy of `buffer`: the tail crossfaded into the head over
 *  `xfadeSecs`, ready for `source.loop = true`. Falls back to the original when
 *  the clip is too short to fold. */
function makeSeamlessLoop(ctx: BaseAudioContext, buffer: AudioBuffer, xfadeSecs: number): AudioBuffer {
  const x = Math.min(Math.floor(xfadeSecs * buffer.sampleRate), Math.floor(buffer.length / 2));
  if (x < 1) return buffer;
  const out = ctx.createBuffer(buffer.numberOfChannels, buffer.length - x, buffer.sampleRate);
  for (let ch = 0; ch < buffer.numberOfChannels; ch++) {
    out.getChannelData(ch).set(crossfadeLoopChannel(buffer.getChannelData(ch), x));
  }
  return out;
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
  // Seamless-loop copies of soundscape buffers, computed once per id.
  private readonly loopBuffers = new Map<string, AudioBuffer>();
  // Manifest fetches are deduped by caching the in-flight promise, so concurrent
  // callers (e.g. list + play at startup) share one request.
  private audioManifest: Promise<AudioManifest | null> | null = null;
  private voiceManifest: Promise<VoiceManifest | null> | null = null;
  private soundscape: Soundscape | null = null;
  // Generation tokens serialize concurrent async starts: a slower-resolving
  // older request bails instead of clobbering the newest one.
  private soundGen = 0;
  private voiceGen = 0;
  private voiceActive = false;
  // True only while a prompt is audibly playing (not the silent gap) — the orb
  // raises its voice rings while this holds, matching the CLI's per-prompt window.
  private voiceSpeaking = false;
  // Only one prompt timer is ever live at a time.
  private voiceTimer = 0;
  private currentBell: string | null = null;
  private muteHinted = false;

  constructor(
    private readonly notice: Notice,
    private readonly touch = false,
  ) {}

  private context(): AudioContext {
    if (!this.ctx) {
      // webkitAudioContext: Safari's vendor prefix, absent from lib.dom types.
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
    const gen = ++this.soundGen;
    const ctx = this.context();
    const buffer = await this.loadOrNotice(audioUrl('soundscape', id));
    // A newer request started while this one was fetching — let it win.
    if (!buffer || gen !== this.soundGen) return;

    const fade = ctx.createGain();
    const duck = ctx.createGain();
    fade.connect(duck).connect(this.master!);
    let looped = this.loopBuffers.get(id);
    if (!looped) {
      looped = makeSeamlessLoop(ctx, buffer, LOOP_CROSSFADE_SECS);
      this.loopBuffers.set(id, looped);
    }
    const source = ctx.createBufferSource();
    source.buffer = looped;
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
    // Release the node graph once the fade-out finishes, so a long session of
    // soundscape switches doesn't accumulate dead-but-connected nodes.
    prev.source.onended = () => {
      prev.source.disconnect();
      prev.fade.disconnect();
      prev.duck.disconnect();
    };
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
    const gen = ++this.voiceGen;
    this.voiceActive = true;
    this.duckTo(DUCK_LEVEL);

    let index = 0;
    const playNext = async (): Promise<void> => {
      const prompt = prompts[index % prompts.length];
      index++;
      const buffer = await this.loadOrNotice(voiceUrl(packId, prompt.id));
      // Bail if stopVoice ran or a newer playVoice superseded this chain.
      if (!buffer || gen !== this.voiceGen) return;
      const ctx = this.context();
      const src = ctx.createBufferSource();
      src.buffer = buffer;
      src.connect(this.master!);
      this.voiceSpeaking = true;
      src.onended = () => {
        src.disconnect();
        if (gen === this.voiceGen) this.voiceSpeaking = false;
      };
      try {
        src.start();
      } catch {
        // A suspended/closed context (e.g. an iOS audio interruption) rejects
        // start(); don't leave the orb's rings raised with no sound, and don't
        // chain the next prompt into a dead context.
        this.voiceSpeaking = false;
        src.disconnect();
        return;
      }
      const gapMs = (buffer.duration + voiceGapSecs()) * 1000;
      this.voiceTimer = window.setTimeout(() => void playNext(), gapMs);
    };
    void playNext();
  }

  stopVoice(): void {
    this.voiceGen++;
    this.voiceActive = false;
    this.voiceSpeaking = false;
    window.clearTimeout(this.voiceTimer);
    this.duckTo(1);
  }

  /** Whether a meditation prompt is audibly speaking right now (not the silent
   *  gap) — the orb raises its vibrating voice rings while this is true. */
  isVoiceSpeaking(): boolean {
    return this.voiceSpeaking;
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

  async listBells(): Promise<AudioAsset[]> {
    const m = await this.audioManifestOrNull();
    return m ? assetsOfType(m, 'bell') : [];
  }

  /** Select the bell that `ring()` (opening + milestone chimes) will use; `null`
   *  falls back to the synth bell. */
  setBell(id: string | null): void {
    this.currentBell = id;
  }

  /** Ring the selected bell, or the synth bell when none/unavailable. */
  async ring(): Promise<void> {
    if (this.currentBell) {
      const buffer = await this.loadOrNotice(audioUrl('bell', this.currentBell));
      if (buffer) {
        const ctx = this.context();
        const src = ctx.createBufferSource();
        src.buffer = buffer;
        src.connect(this.master!);
        src.onended = () => src.disconnect();
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
    env.gain.setValueAtTime(BELL_GAIN, now);
    env.gain.exponentialRampToValueAtTime(BELL_GAIN * Math.exp(-3 * BELL_SECS), now + BELL_SECS);
    BELL_PARTIALS.forEach(([mult, amp], i) => {
      const osc = ctx.createOscillator();
      osc.frequency.value = BELL_BASE_HZ * mult;
      const g = ctx.createGain();
      g.gain.value = amp;
      osc.connect(g).connect(env);
      const isLast = i === BELL_PARTIALS.length - 1;
      osc.onended = () => {
        osc.disconnect();
        g.disconnect();
        if (isLast) env.disconnect();
      };
      osc.start(now);
      osc.stop(now + BELL_SECS);
    });
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

  private audioManifestOrNull(): Promise<AudioManifest | null> {
    if (!this.audioManifest) {
      this.audioManifest = this.fetchManifest(`${AUDIO_BASE}/manifest.json`, (v) => 'assets' in v);
    }
    return this.audioManifest;
  }

  private voiceManifestOrNull(): Promise<VoiceManifest | null> {
    if (!this.voiceManifest) {
      this.voiceManifest = this.fetchManifest(`${VOICE_BASE}/manifest.json`, (v) => 'packs' in v);
    }
    return this.voiceManifest;
  }

  /** Fetch + JSON-parse a manifest, validated by a runtime shape guard (so a
   *  surprise response body can't masquerade as a typed manifest). */
  private async fetchManifest<T>(
    url: string,
    looksValid: (v: Record<string, unknown>) => boolean,
  ): Promise<T | null> {
    try {
      const res = await fetch(url, { mode: 'cors' });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const parsed: unknown = await res.json();
      if (typeof parsed !== 'object' || parsed === null || !looksValid(parsed as Record<string, unknown>)) {
        throw new Error('unexpected manifest shape');
      }
      return parsed as T;
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
