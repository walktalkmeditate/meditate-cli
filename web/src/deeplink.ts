// Shareable sessions via the URL hash fragment (#p=box&snd=rain). The hash is
// never sent to the server, so a deep-link never 404s on GitHub Pages — this
// supersedes the origin doc's `?for=` query example (equivalent for the user,
// server-safe). Human-readable and copy-pasteable.

import { isPattern } from './patterns';

export interface DeepLink {
  pattern?: string;
  sound?: string;
  /** A `p=` value that wasn't a known pattern, so the caller can fall back. */
  invalidPattern?: string;
}

/** A safe sound/asset id: the hash is attacker-controlled (a one-click link),
 *  so anything not in the asset alphabet is rejected before it can reach the
 *  terminal as escape bytes. Real ids are `[a-z0-9-]` (see src/pack safe_component). */
const SAFE_ID = /^[a-z0-9-]{1,32}$/;

/** For the echoed "unknown pattern '…'" message: drop control/ESC bytes and cap
 *  length, so a junk `p=` value can't drive the terminal. */
function neutralize(value: string): string {
  // eslint-disable-next-line no-control-regex
  return value.replace(/[\x00-\x1f\x7f-\x9f]/g, '').slice(0, 32);
}

export function parseHash(hash: string): DeepLink {
  const out: DeepLink = {};
  const params = new URLSearchParams(hash.replace(/^#/, ''));

  const p = params.get('p');
  if (p) {
    if (isPattern(p)) out.pattern = p;
    else out.invalidPattern = neutralize(p);
  }
  const snd = params.get('snd');
  if (snd && SAFE_ID.test(snd)) out.sound = snd;
  return out;
}

export function buildHash(link: DeepLink): string {
  const params = new URLSearchParams();
  if (link.pattern) params.set('p', link.pattern);
  if (link.sound) params.set('snd', link.sound);
  const query = params.toString();
  return query ? `#${query}` : '';
}

export function shareUrl(link: DeepLink, base = `${location.origin}${location.pathname}`): string {
  return `${base}${buildHash(link)}`;
}

/** Whether a deep-link carries anything worth landing pre-configured. */
export function hasConfig(link: DeepLink): boolean {
  return Boolean(link.pattern || link.sound || link.invalidPattern);
}
