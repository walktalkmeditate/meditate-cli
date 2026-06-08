import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { WebglAddon } from '@xterm/addon-webgl';
import '@xterm/xterm/css/xterm.css';

const FONT_STACK =
  '"SF Mono", "JetBrains Mono", "Cascadia Code", Menlo, Consolas, monospace';

/**
 * Build the xterm.js terminal: WebGL for truecolor at 30fps, the fit addon for
 * sizing, a calm dark theme, and the cursor hidden during the animation. The
 * background matches the orb's so the fit margins are invisible.
 */
export function createTerminal(container: HTMLElement): {
  term: Terminal;
  fit: FitAddon;
} {
  const term = new Terminal({
    allowProposedApi: true,
    // Transparent so the constellation backdrop behind the terminal shows
    // through cleared cells. Empty/cleared cells fall back to the body
    // background (#0a0c10), so normal (non-constellation) modes look unchanged;
    // the orb still paints its own opaque per-cell background in block mode.
    allowTransparency: true,
    cursorBlink: true,
    cursorStyle: 'bar',
    fontFamily: FONT_STACK,
    fontSize: 15,
    lineHeight: 1.0,
    scrollback: 0,
    theme: {
      background: 'rgba(10, 12, 16, 0)',
      foreground: '#cfe3d4',
      cursor: '#cfe3d4',
    },
  });

  const fit = new FitAddon();
  term.loadAddon(fit);
  term.open(container);

  // WebGL is essential for truecolor at frame rate; fall back silently to the
  // DOM renderer if the context can't be created (older/headless GPUs).
  try {
    term.loadAddon(new WebglAddon());
  } catch {
    /* DOM renderer fallback */
  }

  fit.fit();
  return { term, fit };
}
