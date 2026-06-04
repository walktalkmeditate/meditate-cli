import './style.css';
import init, { Session } from './wasm/meditate_wasm.js';
import { createTerminal } from './terminal';
import { startBreathing } from './loop';
import { BreathingFavicon } from './favicon';

/** Fade out and remove the zero-JS loading placeholder once the orb is live. */
function dismissLoading(): void {
  const el = document.getElementById('loading');
  if (!el) return;
  el.classList.add('gone');
  setTimeout(() => el.remove(), 600);
}

async function boot(): Promise<void> {
  const reduceMotion = window.matchMedia(
    '(prefers-reduced-motion: reduce)',
  ).matches;

  // Load the Rust breath core. The placeholder stays up through this.
  await init();

  const screen = document.getElementById('screen');
  if (!screen) throw new Error('missing #screen mount');
  const { term, fit } = createTerminal(screen);

  // The core has no clock — inject the browser's month/hour so the palette
  // shifts with season and time of day (R9).
  const now = new Date();
  const session = new Session('calm', now.getMonth() + 1, now.getHours());

  // The breathing browser tab lives in the *icon*: a small canvas orb that
  // pulses with the breath. The title stays a calm phase word (no block bar).
  const favicon = new BreathingFavicon();
  let lastPhase = '';
  const reflectBreath = () => {
    favicon.update(session.scale(), session.glow(), session.palette());
    const phase = session.phaseLabel();
    if (phase !== lastPhase) {
      document.title = `meditate · ${phase}`;
      lastPhase = phase;
    }
  };

  dismissLoading();

  // First-paint affordance: the orb breathes silently and a dim hint invites
  // the first interaction (which becomes the audio-unlock gesture in U5).
  let begun = false;
  const hint = (): string | null =>
    begun ? null : 'press any key to begin';
  term.onData(() => {
    begun = true;
  });

  // Keep the grid sized to the window (and to mobile viewport changes).
  const refit = () => fit.fit();
  window.addEventListener('resize', refit);
  window.visualViewport?.addEventListener('resize', refit);

  startBreathing({ term, fit, session, reduceMotion, hint, afterDraw: reflectBreath });
}

boot().catch((err) => {
  // No telemetry — surface failures to the page itself, calmly.
  const loading = document.getElementById('loading');
  if (loading) {
    loading.classList.remove('gone');
    const span = loading.querySelector('span');
    if (span) span.textContent = 'could not start — please reload';
  }
  console.error(err);
});
