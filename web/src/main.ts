import './style.css';
import init, { Session } from './wasm/meditate_wasm.js';
import { createTerminal } from './terminal';
import { startBreathing } from './loop';
import { BreathingFavicon } from './favicon';
import { SmoothOrb } from './orb-canvas';
import { Repl } from './repl';
import { PATTERNS } from './patterns';
import { moss, dim } from './ansi';
import { renderBoot } from './boot';
import { buildRegistry, runCommand, patternStatus } from './commands';
import type { CommandContext } from './commands';
import { soundCommand, voiceCommand, bellCommand } from './commands/audio';
import { streakCommand, shareCommand, exportCommand } from './commands/streak';
import { AudioEngine } from './audio';
import { Persistence, MIN_SESSION_SECS, localDayKey } from './store';
import { parseHash, hasConfig } from './deeplink';
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
  term.write('\x1b[?25l'); // the REPL renders its own block cursor

  // On a keyboard device, grab focus on load (no click required to type) and
  // re-focus on any click and when the tab regains focus, so keystrokes always
  // land in the terminal. Skipped on touch, where auto-focusing would pop the
  // on-screen keyboard — those users drive with the chip row.
  if (!isTouch()) {
    const refocus = (): void => term.focus();
    refocus();
    document.addEventListener('pointerdown', refocus);
    window.addEventListener('focus', refocus);
  }

  // Persistence + deep-link: precedence is deep-link > saved prefs > default.
  const store = new Persistence();
  const lastVisit = store.lastVisit();
  store.markVisit(Date.now());
  const link = parseHash(window.location.hash);
  const prefs = store.prefs();

  const now = new Date();
  let currentPattern = link.pattern ?? prefs.pattern ?? 'calm';
  let currentSound: string | null = null;
  let pendingSound: string | null = link.sound ?? prefs.sound ?? null;
  const session = new Session(currentPattern, now.getMonth() + 1, now.getHours());

  // The smooth orb (a Canvas-2D overlay) runs its own loop; `graphics` toggles it.
  let smoothMode = prefs.graphics ?? false;
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
  if (prefs.bell) audio.setBell(prefs.bell);

  const registry = buildRegistry([
    soundCommand,
    voiceCommand,
    bellCommand,
    streakCommand,
    shareCommand,
    exportCommand,
  ]);
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
    term.write('\x1b[2J\x1b[H');
  };

  const ctx: CommandContext = {
    session,
    term,
    audio,
    store,
    version: VERSION,
    page: (text) => showPage(text),
    status: setStatus,
    currentPattern: () => currentPattern,
    setPattern: (name) => {
      session.setPattern(name);
      currentPattern = name;
      store.setPref('pattern', name);
      setStatus(patternStatus(name));
    },
    graphicsMode: () => smoothMode,
    setGraphics: (smooth) => {
      smoothMode = smooth;
      store.setPref('graphics', smooth);
    },
    setSound: (id) => {
      currentSound = id;
      pendingSound = null;
      store.setPref('sound', id ?? undefined);
    },
    shareLink: () => ({
      pattern: currentPattern,
      sound: currentSound ?? undefined,
    }),
    visibleCommands: () => registry.list.filter((c) => !c.hidden),
  };

  const dispatch = (line: string): void => {
    Promise.resolve(runCommand(line, registry, ctx)).catch((err) => {
      console.error(err);
      setStatus('something went wrong');
    });
  };

  const interact = (): void => {
    if (begun) return;
    begun = true;
    // The first gesture unlocks the AudioContext (iOS) and starts any sound the
    // deep-link or saved prefs asked for (which couldn't autoplay). Play it
    // directly rather than re-parsing the id as a command line, so a sound id
    // is never interpreted as command syntax.
    audio
      .unlock()
      .then(() => {
        void audio.ring(); // a soft opening chime as the session begins
        if (pendingSound) {
          const id = pendingSound;
          ctx.setSound(id);
          void audio.playSoundscape(id);
        }
      })
      .catch((err) => console.error(err));
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

  // Earn a streak day: accrue active breathing time (visible + not paused) and
  // mark today once it crosses the minimum — mirrors src/streak.rs's threshold.
  // The counter is tied to a day key and reset on rollover, so a tab left open
  // past midnight still earns the new day.
  let countedDay = localDayKey();
  let breathedToday = store.hasToday() ? MIN_SESSION_SECS : 0;
  // A bell punctuates the session at these minute marks (mirrors the CLI).
  const milestonesSecs = [300, 600, 900, 1200, 1800];
  let sessionSecs = 0;
  let milestoneIdx = 0;
  window.setInterval(() => {
    if (paging || session.isPaused() || document.visibilityState !== 'visible') return;
    const today = localDayKey();
    if (today !== countedDay) {
      countedDay = today;
      breathedToday = store.hasToday() ? MIN_SESSION_SECS : 0;
    }
    breathedToday += 1;
    store.addSeconds(1, performance.now());
    if (breathedToday >= MIN_SESSION_SECS && !store.hasToday()) store.markToday();

    sessionSecs += 1;
    if (milestoneIdx < milestonesSecs.length && sessionSecs >= milestonesSecs[milestoneIdx]) {
      milestoneIdx += 1;
      void audio.ring();
    }
  }, 1000);

  if (hasConfig(link)) {
    // A shared link lands pre-configured but waiting — audio can't autoplay.
    const parts = [link.pattern, link.sound].filter(Boolean).join(' · ');
    if (link.invalidPattern) {
      setStatus(`unknown pattern '${link.invalidPattern}' — starting calm`);
    }
    showPage(
      [
        `  shared session${parts ? ': ' + parts : ''}`,
        '',
        '  press any key — or tap — to begin',
      ].join('\n'),
    );
  } else {
    // The login boot: a calm banner that fades to the breathing orb (or any key).
    showPage(renderBoot(VERSION, lastVisit, Date.now()), reduceMotion ? 2200 : 4200);
  }
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
