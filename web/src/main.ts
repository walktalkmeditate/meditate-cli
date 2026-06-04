import './style.css';
import init, { Session } from './wasm/meditate_wasm.js';
import { createTerminal } from './terminal';
import { startBreathing } from './loop';
import { BreathingFavicon } from './favicon';
import { SmoothOrb } from './orb-canvas';
import { Repl } from './repl';
import { PATTERNS } from './patterns';
import { moss, dim } from './ansi';
import { renderMotd } from './motd';
import { buildRegistry, runCommand, patternStatus } from './commands';
import type { CommandContext } from './commands';
import { soundCommand, voiceCommand, bellCommand } from './commands/audio';
import { AudioEngine } from './audio';
import { isTouch, createChipBar } from './mobile';

const VERSION = '0.2.1';
const PROMPT = `${moss('❯')} `;

/** Fade out and remove the zero-JS loading placeholder once the orb is live. */
function dismissLoading(): void {
  const el = document.getElementById('loading');
  if (!el) return;
  el.classList.add('gone');
  setTimeout(() => el.remove(), 600);
}

const toCRLF = (text: string): string => text.replace(/\n/g, '\r\n');

async function boot(): Promise<void> {
  const reduceMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  await init();

  const screen = document.getElementById('screen');
  if (!screen) throw new Error('missing #screen mount');
  const { term, fit } = createTerminal(screen);
  term.write('\x1b[?25l'); // hide xterm's cursor — the REPL draws its own

  const now = new Date();
  let currentPattern = 'calm';
  const session = new Session(currentPattern, now.getMonth() + 1, now.getHours());

  // The smooth orb (a Canvas-2D overlay) runs its own loop; `graphics` toggles it.
  let smoothMode = false;
  const smoothOrb = new SmoothOrb(screen);

  // The breathing browser tab lives in the icon (see favicon.ts).
  const favicon = new BreathingFavicon();
  let lastPhase = '';
  const reflectBreath = (): void => {
    favicon.update(session.scale(), session.glow(), session.palette());
    const phase = session.phaseLabel();
    if (phase !== lastPhase) {
      document.title = `meditate · ${phase}`;
      lastPhase = phase;
    }
  };

  // ── REPL + command surface ─────────────────────────────────────────────────
  let begun = false;
  let paging = false;
  let pageTimer = 0;
  let statusText = '';
  let statusUntil = 0;

  const setStatus = (line: string): void => {
    statusText = line;
    statusUntil = line ? performance.now() + 2600 : 0;
  };

  // Audio loads live from the CDN; failures route to the status line and fall
  // back to the synth bell (see audio.ts).
  const audio = new AudioEngine(setStatus, isTouch());

  const registry = buildRegistry([soundCommand, voiceCommand, bellCommand]);
  const repl = new Repl(() => [...PATTERNS, ...registry.map.keys()]);

  const showPage = (text: string, autoDismissMs = 0): void => {
    paging = true;
    window.clearTimeout(pageTimer);
    term.write(
      '\x1b[2J\x1b[H' + toCRLF(text) + '\r\n\r\n' + dim('  ─ press any key ─'),
    );
    if (autoDismissMs > 0) {
      pageTimer = window.setTimeout(dismissPage, autoDismissMs);
    }
  };

  const dismissPage = (): void => {
    if (!paging) return;
    paging = false;
    window.clearTimeout(pageTimer);
    term.write('\x1b[2J\x1b[H'); // the loop resumes overdrawing from home
  };

  const ctx: CommandContext = {
    session,
    term,
    audio,
    version: VERSION,
    page: (text) => showPage(text),
    status: setStatus,
    currentPattern: () => currentPattern,
    setPattern: (name) => {
      session.setPattern(name);
      currentPattern = name;
      setStatus(patternStatus(name));
    },
    graphicsMode: () => smoothMode,
    setGraphics: (smooth) => {
      smoothMode = smooth;
    },
    commandNames: () => [...registry.map.keys()],
    visibleCommands: () => registry.list.filter((c) => !c.hidden),
  };

  const dispatch = (line: string): void => {
    void runCommand(line, registry, ctx);
  };

  const interact = (): void => {
    begun = true;
    void audio.unlock(); // the first gesture unlocks the AudioContext (iOS)
  };

  term.onData((data) => {
    interact();
    if (paging) {
      dismissPage();
      return;
    }
    const result = repl.handle(data);
    if (result.submitted !== undefined) dispatch(result.submitted);
  });

  // The bottom row: a transient status, else the live prompt.
  const bottomLine = (): string => {
    if (statusText && performance.now() < statusUntil) return '  ' + statusText;
    return '  ' + repl.line(PROMPT, begun ? null : "type 'help'");
  };

  dismissLoading();

  // Touch: a chip row so a session is completable without a keyboard. The set
  // grows as later units add sound / voice / bell / theme / share.
  if (isTouch()) {
    const chips = [
      { label: 'pattern', command: 'next' },
      { label: 'pause', command: 'pause' },
      { label: 'sound', command: 'sound' },
      { label: 'bell', command: 'bell' },
      { label: 'orb', command: 'graphics' },
      { label: 'help', command: 'help' },
      { label: 'install', command: 'install' },
    ];
    document.body.appendChild(
      createChipBar(chips, (cmd) => {
        interact();
        if (paging) dismissPage();
        dispatch(cmd);
      }),
    );
  }

  const refit = (): void => {
    fit.fit();
    smoothOrb.resize();
  };
  window.addEventListener('resize', refit);
  window.visualViewport?.addEventListener('resize', refit);

  startBreathing({
    term,
    fit,
    session,
    reduceMotion,
    bottomLine,
    isPaging: () => paging,
    orbMode: () => (smoothMode ? 'smooth' : 'block'),
    afterDraw: reflectBreath,
  });

  // The smooth orb draws only when in graphics mode and no page is up.
  smoothOrb.start(session, () => smoothMode && !paging);

  // The login MOTD: a brief banner that fades to the breathing orb (or any key).
  showPage(renderMotd(VERSION), reduceMotion ? 2500 : 4200);
}

boot().catch((err) => {
  const loading = document.getElementById('loading');
  if (loading) {
    loading.classList.remove('gone');
    const span = loading.querySelector('span');
    if (span) span.textContent = 'could not start — please reload';
  }
  console.error(err);
});
