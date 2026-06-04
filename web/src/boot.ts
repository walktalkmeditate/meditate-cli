// The boot moment: a calm, login-style banner that fades to the breathing orb.
// Terminal-native (a real shell's "Last login…"), skippable with any key, and
// reduced-motion aware (the caller shortens the dwell). No Matrix-rain.

import { dim } from './ansi';
import { renderMotd } from './motd';

/** A short, friendly elapsed-time phrase for the "Last login" line. */
export function relativeTime(fromMs: number, nowMs: number): string {
  const seconds = Math.max(0, Math.floor((nowMs - fromMs) / 1000));
  if (seconds < 90) return 'just now';
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes} minutes ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} hour${hours === 1 ? '' : 's'} ago`;
  const days = Math.floor(hours / 24);
  return `${days} day${days === 1 ? '' : 's'} ago`;
}

/**
 * The boot page: a login line over the MOTD banner. A returning visitor sees
 * when they last breathed here; a first-timer gets a gentle welcome.
 */
export function renderBoot(version: string, lastVisit: number | null, nowMs: number): string {
  const login =
    lastVisit !== null
      ? `Last login: ${relativeTime(lastVisit, nowMs)} on cli.pilgrimapp.org`
      : 'Welcome — first breath on cli.pilgrimapp.org';
  return `${dim(login)}\n\n${renderMotd(version)}`;
}
