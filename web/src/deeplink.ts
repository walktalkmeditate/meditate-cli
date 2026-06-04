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

export function parseHash(hash: string): DeepLink {
  const out: DeepLink = {};
  const params = new URLSearchParams(hash.replace(/^#/, ''));

  const p = params.get('p');
  if (p) {
    if (isPattern(p)) out.pattern = p;
    else out.invalidPattern = p;
  }
  const snd = params.get('snd');
  if (snd) out.sound = snd;
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
