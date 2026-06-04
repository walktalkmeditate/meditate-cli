import { bold, dim, moss } from './ansi';

/** The login-style banner shown once the orb comes alive — calm, no chrome. */
export function renderMotd(version: string): string {
  return [
    '  ' + moss(bold('meditate')),
    '  ' + dim('─────────'),
    '  ' + dim(`v${version} · local session · no account, nothing leaves your browser`),
    '  ' + dim("type 'help' to begin, or 'install' to run it in your real terminal"),
  ].join('\n');
}
